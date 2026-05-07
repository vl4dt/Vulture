import './styles/global.css';
import { mount } from 'svelte';
import App from './app.svelte';

// Debug: show something immediately
const debug = document.createElement('div');
debug.style.cssText = 'position:fixed;top:10px;right:10px;color:#00f0ff;font-family:monospace;z-index:9999;background:rgba(0,0,0,0.8);padding:8px;border:1px solid #00f0ff;';
debug.textContent = 'JS LOADED';
document.body.appendChild(debug);

const target = document.getElementById('app');
if (!target) {
  debug.textContent = 'ERROR: #app MISSING';
  debug.style.color = 'red';
} else {
  try {
    mount(App, { target });
    debug.textContent = 'MOUNTED OK';
  } catch (e) {
    debug.textContent = `MOUNT ERROR: ${e}`;
    debug.style.color = 'red';
  }
}
