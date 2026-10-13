<script setup lang="ts">
import { ref, computed } from 'vue';
import { X, Plus, Download, Upload, FolderOpen, Pencil, Trash2, Search, Monitor, ChevronRight, KeyRound } from 'lucide-vue-next';
import type { Connection } from '../lib/types';
const props = defineProps<{connections: Connection[]}>();
const emit = defineEmits<{close: []; create: []; edit: [c: Connection]; connect: [c: Connection]; remove: [c: Connection]; import: [directory: boolean]; export: [c?: Connection]}>();
const query = ref(''); const group = ref('全部连接'); const selected = ref<string>();
const groups = computed(() => [...new Set(props.connections.map(c => c.group || '未分组'))].sort());
const visible = computed(() => props.connections.filter(c => (group.value === '全部连接' || (c.group || '未分组') === group.value) && `${c.name} ${c.host} ${c.username}`.toLowerCase().includes(query.value.toLowerCase())));
const current = computed(() => props.connections.find(c => c.id === selected.value));
</script>
<template><div class="overlay" @mousedown.self="emit('close')"><section class="dialog manager-dialog" role="dialog" aria-modal="true" aria-label="连接管理器">
  <div class="dialog-title"><span><Monitor :size="17"/> 连接管理器</span><button class="icon-button" aria-label="关闭连接管理器" @click="emit('close')"><X :size="18"/></button></div>
  <div class="manager-toolbar"><button @click="emit('create')"><Plus :size="16"/> 新建 SSH 连接</button><button @click="emit('import', false)"><Download :size="16"/> 导入连接</button><button @click="emit('import', true)"><FolderOpen :size="16"/> 导入 conn 目录</button><button @click="emit('export', current)"><Upload :size="16"/> 导出</button><div class="search-input"><Search :size="14"/><input v-model="query" placeholder="搜索名称、地址" aria-label="搜索连接"/></div></div>
  <div class="manager-body"><nav class="group-tree"><button :class="{selected: group === '全部连接'}" @click="group='全部连接'"><FolderOpen :size="16"/> 全部连接 <small>{{ connections.length }}</small></button><button v-for="g in groups" :key="g" :class="{selected: group === g}" @click="group=g"><ChevronRight :size="12"/><FolderOpen :size="15"/>{{ g }}</button></nav>
  <div class="connection-list"><table><thead><tr><th>名称</th><th>主机</th><th>端口</th><th>用户名</th><th>认证</th><th></th></tr></thead><tbody><tr v-for="c in visible" :key="c.id" :class="{selected: selected===c.id}" @click="selected=c.id" @dblclick="emit('connect', c)"><td><Monitor :size="15"/>{{ c.name }}</td><td>{{ c.host }}</td><td>{{ c.port }}</td><td>{{ c.username }}</td><td><KeyRound v-if="c.auth==='key'" :size="13"/>{{ c.auth==='key'?'私钥':'密码' }}</td><td class="row-actions"><button class="icon-button" :aria-label="'编辑 '+c.name" @click.stop="emit('edit', c)"><Pencil :size="14"/></button><button class="icon-button" :aria-label="'删除 '+c.name" @click.stop="emit('remove', c)"><Trash2 :size="14"/></button></td></tr></tbody></table><div v-if="!visible.length" class="manager-empty"><FolderOpen :size="38"/><strong>{{ query ? '没有匹配的连接' : '还没有服务器连接' }}</strong><p>新建连接，或导入 FinalShell 的连接 JSON / conn 目录。</p><button class="primary" @click="emit('create')"><Plus :size="15"/> 新建 SSH 连接</button></div></div></div>
  <div class="dialog-footer"><span class="muted">双击连接打开终端</span><button @click="emit('close')">关闭</button><button class="primary" :disabled="!current" @click="current && emit('connect', current)">连接</button></div>
</section></div></template>
