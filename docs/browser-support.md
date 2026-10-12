# Browser support

| Desktop browser | Minimum version | Release year |
| --- | --- | --- |
| Chrome | 105 | 2022 |
| Edge | 105 | 2022 |
| Firefox | 121 | 2023 |
| Safari | 16.4 | 2023 |

These minimums are inferred from the features Gebakuk uses, rather than tests of
each historical browser version. Use a current release. See
[verification](verification.md) for the browsers actually exercised.

The layout uses [CSS `:has()`](https://github.com/mdn/browser-compat-data/blob/main/css/selectors/has.json),
which sets the minimum for Chrome, Edge, and Firefox. Safari 16.4 supplies
[WebAssembly SIMD](https://github.com/mdn/browser-compat-data/blob/main/webassembly/fixed-width-SIMD.json)
and the unprefixed
[fullscreen APIs](https://github.com/mdn/browser-compat-data/blob/main/api/Element.json).
Other requirements, including `inert`, Fetch streams, and WebAssembly reference
types, are available by these versions. The table covers desktop browsers;
mobile fullscreen and audience-window behavior depend on the device.

Release dates: [Chrome 105](https://chromereleases.googleblog.com/2022/08/stable-channel-update-for-desktop_30.html),
[Edge 105](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-relnote-archive-stable-channel#version-1050134325-september-1-2022),
[Firefox 121](https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/121),
and [Safari 16.4](https://webkit.org/blog/13966/webkit-features-in-safari-16-4/).
