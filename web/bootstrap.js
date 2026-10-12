import init from './pkg/presenter_web.js';

init().catch((error) => {
  document.getElementById('app').textContent = `Unable to start the presenter: ${error}`;
});
