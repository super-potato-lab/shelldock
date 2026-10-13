<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref, watch } from 'vue';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import '@xterm/xterm/css/xterm.css';
const props = defineProps<{active: boolean; connected: boolean; fontSize: number; light: boolean}>();
const emit = defineEmits<{input: [data: string]; resize: [columns: number, rows: number]; consumed: [bytes: number]; directory: [path: string]}>();
const element = ref<HTMLDivElement>(); let term: Terminal; let fit: FitAddon; let search: SearchAddon; let observer: ResizeObserver;
const queued: {data: Uint8Array, ack: boolean}[] = [];
let resizer: ReturnType<typeof setTimeout>;
function fitNow() { if (!props.active || !term) return; clearTimeout(resizer); resizer = setTimeout(() => { fit.fit(); emit('resize', term.cols, term.rows); }, 70); }
function write(data: Uint8Array, ack = true) {
  if (!term) { queued.push({data, ack}); return; }
  term.write(data, () => { if (ack) emit('consumed', data.length); });
}
function note(text: string) { write(new TextEncoder().encode(`\r\n\x1b[38;5;244m${text}\x1b[0m\r\n`), false); }
function find(value: string) { if (value) search?.findNext(value); }
function focus() { term?.focus(); }
function dimensions() { return {columns: term?.cols || 100, rows: term?.rows || 30}; }
defineExpose({write, note, find, focus, dimensions});
onMounted(() => {
  term = new Terminal({fontFamily: 'Consolas, "Cascadia Mono", "Liberation Mono", monospace', fontSize: props.fontSize, cursorBlink: true, scrollback: 10000, allowProposedApi: false, theme: {background: '#101010', foreground: '#e6e6e6', cursor: '#eeeeee'}});
  fit = new FitAddon(); search = new SearchAddon(); term.loadAddon(fit); term.loadAddon(search); term.open(element.value!);
  term.onData(data => { if (props.connected) emit('input', data); });
  term.parser.registerOscHandler(7, data => { try { const url = new URL(data); if (url.protocol === 'file:') emit('directory', decodeURIComponent(url.pathname)); } catch { /* unrelated OSC */ } return true; });
  observer = new ResizeObserver(fitNow); observer.observe(element.value!); fitNow();
  for (const q of queued.splice(0)) write(q.data, q.ack);
});
watch(() => props.active, active => { if (active) { fitNow(); setTimeout(focus, 80); } });
watch(() => props.fontSize, value => { if (term) { term.options.fontSize = value; fitNow(); } });
watch(() => props.light, light => { if (term) term.options.theme = light ? {background: '#fafafa', foreground: '#202020', cursor: '#222'} : {background: '#101010', foreground: '#e6e6e6', cursor: '#eee'}; });
onBeforeUnmount(() => { clearTimeout(resizer); observer?.disconnect(); term?.dispose(); });
</script>
<template><div class="terminal-view" ref="element" /></template>
