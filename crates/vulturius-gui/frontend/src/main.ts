import './styles/global.css';
import { mount } from 'svelte';
import App from './app.svelte';

console.log('Vulturius: mounting app...');
mount(App, {
  target: document.getElementById('app')!,
});
console.log('Vulturius: app mounted');
