#!/usr/bin/env python3
"""Exercise presenter/audience layout and navigation against a built dist/."""

import functools
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import shutil
import threading

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]


class Handler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


def settled(page, slide=1, notes=False, gifs=None):
    page.wait_for_function(
        """args => {
          const label = 'Slide ' + args.slide;
          const canvas = document.querySelector('#current-slide canvas');
          const note = document.querySelector('#notes-slide canvas');
          const images = [...document.querySelectorAll('.gif-overlay')];
          return canvas?.getAttribute('aria-label') === label
            && (!args.notes || note?.getAttribute('aria-label') === label)
            && (args.gifs === null || images.length === args.gifs)
            && images.every(image => image.complete && image.naturalWidth > 0);
        }""",
        arg={"slide": slide, "notes": notes, "gifs": gifs},
    )


def metrics(page):
    return page.evaluate("""() => {
      const result = {width: innerWidth, height: innerHeight,
        scrollWidth: document.documentElement.scrollWidth,
        focused: !!document.querySelector('.presenter-shell[data-notes-focused]')};
      for (const name of ['current', 'notes', 'next']) {
        const host = document.querySelector('#' + name + '-slide');
        const canvas = host?.querySelector('canvas');
        const rect = host?.getBoundingClientRect();
        const surface = host?.querySelector('.slide-surface')?.getBoundingClientRect();
        result[name] = rect ? {x: rect.x, y: rect.y, width: rect.width,
          height: rect.height, surfaceWidth: surface?.width ?? 0,
          surfaceHeight: surface?.height ?? 0, pixels: canvas?.width ?? 0} : null;
      }
      return result;
    }""")


def focus_ready(page):
    page.locator(".presenter-shell[data-notes-focused]").wait_for()
    settled(page, notes=True)
    page.wait_for_function("""() => {
      const host = document.querySelector('#notes-slide');
      const surface = host.querySelector('.slide-surface');
      return surface && surface.getBoundingClientRect().width >
        host.getBoundingClientRect().width * 0.9;
    }""")


def fit_ready(page):
    page.wait_for_function("""() => ['current', 'notes', 'next'].every(name => {
      const host = document.querySelector('#' + name + '-slide');
      const surface = host?.querySelector('.slide-surface');
      if (!surface) return false;
      const bounds = host.getBoundingClientRect();
      const actual = surface.getBoundingClientRect();
      const expected = Math.min(bounds.width, bounds.height * 16 / 9);
      return Math.abs(actual.width - expected) < 2;
    })""")


def load_pdf(page, name, pages):
    page.locator("#file-input").set_input_files(ROOT / "fixtures" / name)
    page.wait_for_function(
        "count => document.querySelector('#slide-count').textContent === '1 / ' + count",
        arg=pages,
    )
    page.locator("#pdf-loader").wait_for(state="hidden")


def fullscreen(page, notes=False):
    page.locator("#fullscreen-button").click()
    page.wait_for_function("document.fullscreenElement !== null")
    if notes:
        assert page.evaluate(
            "document.fullscreenElement.contains(document.querySelector('#notes-slide'))"
        ), "Presenter fullscreen hid the speaker notes"
    page.locator("#fullscreen-button").click()
    page.wait_for_function("document.fullscreenElement === null")


def main():
    executable = shutil.which("google-chrome") or shutil.which("chromium")
    assert executable, "Install Google Chrome or Chromium to run browser checks"
    server = ThreadingHTTPServer(
        ("127.0.0.1", 0), functools.partial(Handler, directory=str(ROOT / "dist"))
    )
    threading.Thread(target=server.serve_forever, daemon=True).start()
    errors, results = [], {}
    try:
        with sync_playwright() as playwright:
            browser = playwright.chromium.launch(
                executable_path=executable,
                args=["--no-sandbox", "--disable-background-timer-throttling",
                      "--disable-renderer-backgrounding"],
            )
            context = browser.new_context(
                viewport={"width": 1440, "height": 900}, device_scale_factor=1
            )
            context.set_default_timeout(15000)
            context.add_init_script("""const nativeOpen = window.open.bind(window);
              window.open = (...args) => window.__blockPopup ? null : nativeOpen(...args);""")
            context.on("page", lambda page: page.on("pageerror", lambda e: errors.append(str(e))))
            page = context.new_page()
            page.goto(f"http://127.0.0.1:{server.server_port}/?source=guide.pdf")
            settled(page, notes=True, gifs=1)
            before = metrics(page)
            page.evaluate("window.__blockPopup = true")
            page.locator("#audience-button").click()
            page.wait_for_function("document.querySelector('#status').textContent.includes('blocked')")
            assert not metrics(page)["focused"], "Blocked popup changed presenter layout"
            page.evaluate("window.__blockPopup = false")
            with page.expect_popup() as popup:
                page.locator("#audience-button").click()
            audience = popup.value
            audience.set_viewport_size({"width": 1024, "height": 768})
            settled(audience, gifs=1)
            assert audience.locator("#notes-slide").count() == 0
            page.bring_to_front()
            focus_ready(page)
            after = metrics(page)
            assert after["notes"]["surfaceWidth"] > before["notes"]["surfaceWidth"] * 1.5, (before, after)
            assert after["notes"]["width"] > after["current"]["width"] * 1.3, after
            assert after["notes"]["pixels"] >= after["notes"]["surfaceWidth"] - 1, after
            results["enlargement"] = {"before": before, "after": after}
            page.evaluate("""window.__stableNodes =
              [...document.querySelectorAll('.slide-surface canvas,.gif-overlay')];""")
            page.wait_for_timeout(2200)
            assert page.evaluate("""window.__stableNodes.every((node, i) =>
              node === document.querySelectorAll('.slide-surface canvas,.gif-overlay')[i])"""), \
                "Unchanged timer ticks restarted rendering"
            page.locator("#notes-slide").click()
            page.keyboard.press("ArrowRight")
            settled(page, 2, notes=True, gifs=0)
            settled(audience, 2, gifs=0)
            page.keyboard.press("Home")
            settled(page, notes=True, gifs=1)
            settled(audience, gifs=1)
            fullscreen(page, notes=True)
            audience.bring_to_front()
            fullscreen(audience)
            page.bring_to_front()
            page.locator("#layout-select").select_option("normal")
            page.wait_for_function("!document.querySelector('.presenter-shell[data-notes-focused]')")
            page.locator("#layout-select").select_option("auto")
            focus_ready(page)
            page.locator("#open-document-button").click()
            page.locator("#pdf-loader").wait_for(state="visible")
            assert not metrics(page)["focused"], "PDF opener stayed inside a small preview"
            assert page.locator("#file-open-button").bounding_box()["width"] >= 250
            page.locator("#loader-close-button").click()
            focus_ready(page)
            load_pdf(page, "plain.pdf", 2)
            settled(page, gifs=0)
            settled(audience, gifs=0)
            assert not metrics(page)["focused"], "Ordinary PDF enlarged empty notes"
            page.locator("#fullscreen-button").click()
            page.wait_for_function("document.fullscreenElement !== null")
            page.locator("#file-input").set_input_files(ROOT / "web" / "guide.pdf")
            page.wait_for_function("document.querySelector('#slide-count').textContent === '1 / 8'")
            page.wait_for_function("document.fullscreenElement === null")
            focus_ready(page)
            load_pdf(page, "notes-right.pdf", 3)
            focus_ready(page)
            page.locator("#next-button").click()
            settled(page, 2, notes=True, gifs=2)
            settled(audience, 2, gifs=2)
            load_pdf(page, "transparent-notes-right.pdf", 1)
            focus_ready(page)
            settled(page, notes=True, gifs=2)
            settled(audience, gifs=1)
            for host in ("current", "notes"):
                alignment = page.locator(f"#{host}-slide").evaluate("""host => {
                  const surface = host.querySelector('.slide-surface').getBoundingClientRect();
                  const image = host.querySelector('.gif-overlay').getBoundingClientRect();
                  return {x: (image.x - surface.x) / surface.width,
                    y: (image.y - surface.y) / surface.height,
                    width: image.width / surface.width};
                }""")
                assert abs(alignment["x"] - 40 / 640) < 0.01, alignment
                assert abs(alignment["y"] - 60 / 360) < 0.01, alignment
                assert abs(alignment["width"] - 0.5) < 0.01, alignment
            page.locator("#file-input").set_input_files(ROOT / "web" / "guide.pdf")
            page.wait_for_function("document.querySelector('#slide-count').textContent === '1 / 8'")
            focus_ready(page)
            results["responsive"] = []
            for width, height in [(1440, 900), (1024, 768), (900, 760),
                                  (540, 800), (375, 812), (320, 780), (1440, 600)]:
                page.set_viewport_size({"width": width, "height": height})
                page.evaluate("scrollTo(0, 0)")
                settled(page, notes=True)
                fit_ready(page)
                state = metrics(page)
                assert state["scrollWidth"] <= width + 1, state
                assert state["notes"]["surfaceWidth"] <= state["notes"]["width"] + 1, state
                assert state["notes"]["pixels"] >= state["notes"]["surfaceWidth"] - 1, state
                if width > 900:
                    assert state["notes"]["x"] < state["current"]["x"], state
                else:
                    assert state["notes"]["y"] < state["current"]["y"], state
                    if width <= 540:
                        assert state["current"]["y"] < state["next"]["y"], state
                results["responsive"].append(state)
            page.set_viewport_size({"width": 1440, "height": 900})
            page.locator("#notes-slide").click()
            page.keyboard.press("ArrowRight")
            settled(page, 2, notes=True)
            settled(audience, 2, gifs=0)
            audience.close()
            page.wait_for_function("!document.querySelector('.presenter-shell[data-notes-focused]')")
            with page.expect_popup() as popup:
                page.locator("#audience-button").click()
            audience = popup.value
            settled(audience, 2, gifs=0)
            page.bring_to_front()
            page.wait_for_function("!!document.querySelector('.presenter-shell[data-notes-focused]')")
            settled(page, 2, notes=True)
            assert not errors, errors
            results.update(browser=browser.version, page_errors=errors, status="passed")
            print(json.dumps(results, indent=2), flush=True)
            browser.close()
    finally:
        server.shutdown()
        server.server_close()


if __name__ == "__main__":
    main()
