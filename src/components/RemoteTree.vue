<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount } from 'vue';
import { ChevronRight } from 'lucide-vue-next';
import type { FileEntry } from '../lib/types';
import { call } from '../lib/api';
const props = defineProps<{sessionId?: string; path: string}>();
const emit = defineEmits<{navigate: [path: string]}>();
const children = ref<Record<string, FileEntry[]>>({}); const expanded = ref(new Set<string>(['/']));
const loading = ref(new Set<string>()); const errors = ref<Record<string,string>>({});
let generation = 0; let pathGeneration = 0;
const pending = new Map<string, Promise<void>>();
const rows = computed(() => {
  const result: {path: string; name: string; depth: number}[] = [];
  function visit(path: string, name: string, depth: number) {
    result.push({path, name, depth});
    if (expanded.value.has(path) && depth < 32) for (const child of children.value[path] || []) visit(child.path, child.name, depth+1);
  }
  visit('/', '/', 0); return result;
});
async function load(path: string) {
  if (!props.sessionId || children.value[path]) return;
  if (pending.has(path)) return pending.get(path);
  const run = generation; const sessionId = props.sessionId;
  loading.value.add(path); delete errors.value[path];
  const task = (async () => {
    try { const files = await call<FileEntry[]>('list_files', {sessionId, path}); if (run === generation) children.value[path] = files.filter(f=>f.isDir).sort((a,b)=>a.name.localeCompare(b.name)); }
    catch(e) { if (run === generation) errors.value[path] = String(e); }
    finally { if (run === generation) { loading.value.delete(path); pending.delete(path); } }
  })();
  pending.set(path,task); return task;
}
async function toggle(path: string) { if (expanded.value.has(path)) expanded.value.delete(path); else { expanded.value.add(path); await load(path); } }
async function reveal() {
  const run = ++pathGeneration; const session = generation;
  if (!props.sessionId) return;
  const parts = props.path.split('/').filter(Boolean).slice(0,32); let parent = '/';
  expanded.value.add('/'); await load('/');
  for (const name of parts.slice(0,-1)) {
    if (run !== pathGeneration || session !== generation) return;
    const next = children.value[parent]?.find(f=>f.name===name); if (!next) break;
    parent = next.path; expanded.value.add(parent); await load(parent);
  }
}
async function refresh() { generation++; children.value = {}; pending.clear(); loading.value.clear(); errors.value = {}; await reveal(); await Promise.all([...expanded.value].slice(0,100).map(load)); }
watch(()=>props.sessionId,()=>{generation++; pathGeneration++; children.value={}; expanded.value=new Set(['/']); pending.clear(); loading.value.clear(); errors.value={}; void reveal();},{immediate:true});
watch(()=>props.path,()=>void reveal());
onBeforeUnmount(()=>{generation++;pathGeneration++;});
defineExpose({refresh});
</script>
<template>
  <div class="remote-tree" role="tree" aria-label="远端目录树">
    <div v-for="row in rows" :key="row.path" class="tree-node" :class="{selected:path===row.path}" role="treeitem" :aria-level="row.depth+1" :aria-expanded="expanded.has(row.path)" :aria-selected="path===row.path" :style="{paddingLeft:row.depth*17+'px'}" :title="errors[row.path] || row.path">
      <button class="tree-toggle" :aria-label="(expanded.has(row.path)?'折叠 ':'展开 ')+row.path" :disabled="loading.has(row.path)" @click="toggle(row.path)"><ChevronRight :size="12" :class="{expanded:expanded.has(row.path)}"/></button>
      <button class="tree-directory" :aria-label="'打开目录 '+row.path" @click="emit('navigate',row.path)"><span class="folder-glyph"/>{{ row.name }}</button>
    </div>
    <p v-if="errors['/']" class="tree-error">{{ errors['/'] }} <button @click="load('/')">重试</button></p>
  </div>
</template>
