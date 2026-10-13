<script setup lang="ts">
import { ref, computed, onMounted, onBeforeUnmount, nextTick } from 'vue';
import { Channel } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open, save, confirm } from '@tauri-apps/plugin-dialog';
import { Monitor, Plus, FolderOpen, Download, Upload, Settings2, Search, RefreshCw, Unplug, X, ChevronDown, Terminal, ArrowRight, Check, Info, Pencil, Moon, Sun, Save } from 'lucide-vue-next';
import ConnectionManager from './components/ConnectionManager.vue';
import ConnectionForm from './components/ConnectionForm.vue';
import MonitorPanel from './components/MonitorPanel.vue';
import FilePanel from './components/FilePanel.vue';
import TerminalView from './components/TerminalView.vue';
import RemoteEditor from './components/RemoteEditor.vue';
import { call, desktop } from './lib/api';
import { bytes, joinRemote } from './lib/format';
import type { Connection, Tab, SessionEvent, ConnectResult, Transfer, RemoteText, FileEntry } from './lib/types';

type TerminalRef = InstanceType<typeof TerminalView>;
const connections = ref<Connection[]>([]); const tabs = ref<Tab[]>([]); const activeId = ref('welcome');
const active = computed(() => tabs.value.find(t => t.id === activeId.value));
const manager = ref(false); const form = ref<{connection?: Connection} | null>(null);
const showTransfers = ref(false); const transfers = ref<Transfer[]>([]);
const editor = ref<{sessionId: string; text: RemoteText} | null>(null);
const exportDialog = ref<{connection?: Connection; format: string; includePasswords: boolean} | null>(null);
const promptDialog = ref<{title: string; value: string; type: string; help?: string; complete: (value: string | null) => void} | null>(null);
const about = ref(false); const searchVisible = ref(false); const searchText = ref(''); const fontSize = ref(14); const terminalLight = ref(false);
const command = ref(''); const history = ref<string[]>([]); const quickCommands = ref<string[]>([]); const quickEditor = ref(false); const quickText = ref('');
const notice = ref(''); const noticeError = ref(false); let noticeTimer: ReturnType<typeof setTimeout>;
const terminalRefs = new Map<string, TerminalRef>(); const fileRef = ref<InstanceType<typeof FilePanel>>();
const busy = ref(false); const followDirectory = ref(true); const fileHeight = ref(265); const workspace = ref<HTMLElement>();
let metricsTimer: ReturnType<typeof setTimeout>; let ackTimer: ReturnType<typeof setInterval>; let unlisten: UnlistenFn | undefined;
let disposed = false;
const inputChains = new Map<string, Promise<void>>();
const runningTransfers = computed(() => transfers.value.filter(t => ['queued', 'running'].includes(t.status)).length);
function toast(text: string, error = false) { notice.value = text; noticeError.value = error; clearTimeout(noticeTimer); noticeTimer = setTimeout(() => notice.value = '', error ? 10000 : 4500); }
async function run(task: () => Promise<unknown>) { try { await task(); } catch (e) { toast(String(e), true); } }
function ask(title: string, value = '', type = 'text', help?: string): Promise<string | null> { return new Promise(resolve => { promptDialog.value = {title, value, type, help, complete: resolve}; }); }
function answerPrompt(value: string | null) { const p = promptDialog.value; promptDialog.value = null; p?.complete(value); }
async function question(message: string, title: string): Promise<boolean> { return desktop ? confirm(message, {title, kind: 'warning'}) : window.confirm(message); }
function terminalRef(id: string, value: unknown) { if (value) terminalRefs.set(id, value as TerminalRef); else terminalRefs.delete(id); }
function onSaved(c: Connection) { connections.value = [...connections.value.filter(p => p.id !== c.id), c]; form.value = null; toast(desktop ? '连接已保存' : '界面预览：连接已加入列表，真实连接需运行桌面版本'); }
function createConnection() { form.value = {}; }
async function removeConnection(c: Connection) { if (await question(`删除连接“${c.name}”？`, '删除连接')) { await call('delete_connection', {id: c.id}); connections.value = connections.value.filter(x => x.id !== c.id); } }
async function doImport(directory: boolean) { if (!desktop) { toast('导入功能需要在桌面应用中使用', true); return; } const paths = await open({title: directory ? '选择 FinalShell 数据目录或 conn 目录' : '选择连接配置 JSON', directory, multiple: !directory, ...(directory ? {} : {filters: [{name: '连接配置', extensions: ['json']}]})}); if (!paths) return; busy.value = true; try { const result = await call<{connections: Connection[]; warnings: string[]; imported: number}>('import_connections', {paths: Array.isArray(paths) ? paths : [paths]}); connections.value.push(...result.connections); manager.value = true; toast(`已导入 ${result.imported} 个连接${result.warnings.length ? '。' + result.warnings.join('；') : ''}`, result.warnings.length > 0); } finally { busy.value = false; } }
function prepareExport(c?: Connection) { exportDialog.value = {connection: c, format: c ? 'finalshell' : 'shelldock', includePasswords: false}; }
async function doExport() { const data = exportDialog.value; if (!data) return; if (!desktop) { toast('导出功能需要在桌面应用中使用', true); return; } const c = data.connection; const name = data.format === 'finalshell' && c ? `${c.name.replace(/[\\/:*?"<>|]/g, '_')}_connect_config.json` : 'shelldock-connections.json'; const path = await save({title: '导出连接', defaultPath: name, filters: [{name: 'JSON', extensions: ['json']}]}); if (!path) return; const count = await call<number>('export_connections', {path, format: data.format, ids: c ? [c.id] : [], includePasswords: data.includePasswords}); exportDialog.value = null; toast(`已导出 ${count} 个连接${data.includePasswords ? '（包含兼容格式密码）' : '（不含密码）'}`); }

async function openConnection(c: Connection) {
  manager.value = false;
  if (!desktop) { toast('浏览器为界面预览，请运行桌面版本以连接服务器', true); return; }
  const tab: Tab = {id: crypto.randomUUID(), connection: c, status: 'idle', path: '/', metrics: null, metricError: '', pendingAck: 0};
  tabs.value.push(tab); activeId.value = tab.id; await nextTick(); await establish(tab.id);
}
async function establish(id: string) {
  const t = tabs.value.find(t => t.id === id); if (!t) return;
  t.status = 'connecting'; t.metricError = ''; terminalRefs.get(id)?.note(`正在连接 ${t.connection.host}:${t.connection.port} …`);
  let password: string | null = null;
  if (!t.connection.hasPassword && t.connection.auth === 'password') { password = await ask('SSH 登录密码', '', 'password', `${t.connection.username}@${t.connection.host} · 本次连接使用`); if (password === null) { t.status = 'closed'; return; } }
  if (t.connection.auth === 'key' && !t.connection.hasPassword) { password = await ask('私钥口令', '', 'password', '未加密私钥可直接留空'); if (password === null) { t.status = 'closed'; return; } }
  try {
    for (let attempt = 0; attempt < 2; attempt++) {
      const stream = new Channel<SessionEvent>();
      stream.onmessage = event => {
        if (event.event === 'output') terminalRefs.get(id)?.write(new Uint8Array(event.data));
        else if (event.event === 'error') { t.status = 'error'; terminalRefs.get(id)?.note(event.data); toast(event.data, true); }
        else { t.status = 'closed'; terminalRefs.get(id)?.note('连接已关闭'); if (t.sessionId) void call('disconnect', {sessionId: t.sessionId}).catch(() => {}); }
      };
      const size = terminalRefs.get(id)?.dimensions() || {columns: 100, rows: 30};
      const result = await call<ConnectResult>('connect', {id: t.connection.id, password, ...size, onEvent: stream});
      if (result.status === 'needsTrust') {
        const accept = await question(`${result.changed ? '警告：主机密钥与已保存的指纹不同。' : '首次连接此主机。'}\n${t.connection.host}:${t.connection.port}\n${result.fingerprint}\n\n确认此指纹可信并继续？`, result.changed ? '主机密钥已改变' : '验证主机身份');
        if (!accept) { t.status = 'closed'; return; }
        await call('trust_host', {id: t.connection.id, fingerprint: result.fingerprint}); continue;
      }
      t.sessionId = result.sessionId; t.path = result.path; t.status = 'connected'; terminalRefs.get(id)?.focus(); return;
    }
    throw new Error('主机身份校验未完成');
  } catch (e) { t.status = 'error'; terminalRefs.get(id)?.note(String(e)); toast(String(e), true); }
  finally { password = null; }
}
async function closeTab(id: string) { const t = tabs.value.find(t => t.id === id); if (t?.sessionId) await call('disconnect', {sessionId: t.sessionId}); tabs.value = tabs.value.filter(t => t.id !== id); inputChains.delete(id); if (activeId.value === id) activeId.value = tabs.value.at(-1)?.id || 'welcome'; }
async function disconnectActive() { const t = active.value; if (!t?.sessionId) return; await call('disconnect', {sessionId: t.sessionId}); t.status = 'closed'; }
async function reconnect() { const t = active.value; if (!t || t.status === 'connecting') return; if (t.sessionId) await call('disconnect', {sessionId: t.sessionId}); t.sessionId = undefined; t.metrics = null; await establish(t.id); }
function sendInput(id: string, value: string) { const tab = tabs.value.find(t => t.id === id); if (!tab?.sessionId || tab.status !== 'connected') return; const sessionId = tab.sessionId; const data = new TextEncoder().encode(value); const chain = inputChains.get(id) || Promise.resolve(); inputChains.set(id, chain.then(async () => { for (let offset = 0; offset < data.length; offset += 16384) await call('terminal_input', {sessionId, data: Array.from(data.subarray(offset, offset + 16384))}); }).catch(e => toast(String(e), true))); }
function resizeTerminal(id: string, columns: number, rows: number) { const t = tabs.value.find(t => t.id === id); if (t?.sessionId && t.status === 'connected') void call('terminal_resize', {sessionId: t.sessionId, columns, rows}).catch(e => toast(String(e), true)); }
function consumed(id: string, count: number) { const tab = tabs.value.find(t => t.id === id); if (tab) tab.pendingAck += count; }
function onDirectory(id: string, path: string) { const tab = tabs.value.find(t => t.id === id); if (tab && followDirectory.value && path.startsWith('/')) tab.path = path; }
async function pollMetrics() { if (disposed) return; const t = active.value; if (t?.sessionId && t.status === 'connected') { try { t.metrics = await call('sample_metrics', {sessionId: t.sessionId}); t.metricError = ''; } catch (e) { t.metricError = String(e); } } if (!disposed) metricsTimer = setTimeout(pollMetrics, 2500); }
async function executeCommand() { const t = active.value; if (!t || t.status !== 'connected') { toast('请先连接服务器', true); return; } const value = command.value.trim(); if (!value) return; sendInput(t.id, value + '\r'); await call('record_command', {command: value}); history.value = [value, ...history.value.filter(c => c !== value)].slice(0, 500); command.value = ''; terminalRefs.get(t.id)?.focus(); }
async function saveQuickCommands() { quickCommands.value = quickText.value.split('\n').map(v => v.trim()).filter(Boolean); await call('save_quick_commands', {commands: quickCommands.value}); quickEditor.value = false; }
async function openEditor(file: FileEntry) { const t = active.value; if (!t?.sessionId) return; editor.value = {sessionId: t.sessionId, text: await call('read_remote_text', {sessionId: t.sessionId, path: file.path})}; }
async function closeEditor(dirty: boolean) { if (!dirty || await question('此文件有未保存的修改，确定关闭？', '未保存修改')) editor.value = null; }
async function fileAction(action: string, file?: FileEntry) {
  const t = active.value; if (!t?.sessionId) return; let path = file?.path || t.path; let target: string | null = null;
  if (action === 'delete') { if (!await question(`删除“${file?.name}”？文件夹必须为空。`, '删除远端文件')) return; }
  else if (action === 'mkdir') { const name = await ask('新建远端文件夹'); if (!name) return; if (name.includes('/') || name === '.' || name === '..') throw new Error('请输入单个文件夹名称'); path = joinRemote(t.path, name); }
  else if (action === 'rename') { const name = await ask('重命名', file?.name); if (!name) return; if (name.includes('/') || name === '.' || name === '..') throw new Error('请输入单个文件名称'); target = joinRemote(t.path, name); }
  else if (action === 'chmod') { target = await ask('修改文件权限', file?.permissions == null ? '644' : (file.permissions & 0o7777).toString(8), 'text', '例如：644 或 755'); if (!target) return; if (!/^[0-7]{3,4}$/.test(target)) throw new Error('权限必须是 3 或 4 位八进制数字'); }
  await call('file_action', {sessionId: t.sessionId, action, path, target}); await fileRef.value?.refresh();
}
async function uploadFiles() { const t = active.value; if (!t?.sessionId) return; const files = await open({title: '选择上传文件', multiple: true, directory: false}); if (!files) return; for (const local of Array.isArray(files) ? files : [files]) { const name = local.split(/[\\/]/).at(-1)!; await call('start_transfer', {sessionId: t.sessionId, direction: 'upload', local, remote: joinRemote(t.path, name)}); } showTransfers.value = true; }
async function downloadFile(file: FileEntry) { const t = active.value; if (!t?.sessionId) return; const local = await save({title: '下载文件', defaultPath: file.name}); if (!local) return; await call('start_transfer', {sessionId: t.sessionId, direction: 'download', local, remote: file.path}); showTransfers.value = true; }
function startResize(event: PointerEvent) { const base = fileHeight.value; const y = event.clientY; const move = (e: PointerEvent) => fileHeight.value = Math.max(150, Math.min((workspace.value?.clientHeight || 600) - 220, base + y - e.clientY)); const stop = () => { window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', stop); }; window.addEventListener('pointermove', move); window.addEventListener('pointerup', stop, {once: true}); }
const statusName = computed(() => ({idle:'未连接',connecting:'连接中',connected:'已连接',closed:'已断开',error:'连接失败'}[active.value?.status || 'idle']));
onMounted(async () => { await run(async () => { const data = await call<{connections: Connection[];history: string[];quickCommands: string[]}>('bootstrap'); connections.value = data.connections; history.value = data.history; quickCommands.value = data.quickCommands; if (desktop) unlisten = await listen<Transfer>('transfer-progress', event => { const row = transfers.value.find(r => r.id === event.payload.id); if (row) Object.assign(row, event.payload); else transfers.value.unshift(event.payload); if (event.payload.status === 'completed') void fileRef.value?.refresh(); }); }); void pollMetrics(); ackTimer = setInterval(() => { for (const t of tabs.value) if (t.sessionId && t.pendingAck > 0) { const count = t.pendingAck; t.pendingAck = 0; void call('terminal_ack', {sessionId: t.sessionId, bytes: count}).catch(() => {}); } }, 100); });
onBeforeUnmount(() => { disposed = true; clearTimeout(metricsTimer); clearInterval(ackTimer); clearTimeout(noticeTimer); unlisten?.(); });
</script>

<template><div class="app-shell">
  <header class="menubar"><span class="brand"><span class="brand-mark">S</span> ShellDock</span><button @click="manager=true">文件</button><button @click="manager=true">连接</button><button @click="quickEditor=true;quickText=quickCommands.join('\n')">工具</button><button @click="terminalLight=!terminalLight">选项</button><button @click="about=true">帮助</button><span class="flex-spacer"/><span v-if="!desktop" class="preview-badge">界面预览</span><button class="menu-action" @click="manager=true"><Monitor :size="15"/> 连接管理器</button><button class="icon-button" title="新建连接" @click="createConnection"><Plus :size="17"/></button></header>
  <div class="session-tabs"><button class="session-tab" :class="{active:activeId==='welcome'}" @click="activeId='welcome'"><Monitor :size="14"/> 快速连接</button><div v-for="t in tabs" :key="t.id" class="session-tab" :class="{active:activeId===t.id}" @click="activeId=t.id"><span class="status-dot" :class="{online:t.status==='connected',connecting:t.status==='connecting'}"/><span>{{ t.connection.name }}</span><button class="tab-close" :aria-label="'关闭 '+t.connection.name" @click.stop="run(()=>closeTab(t.id))"><X :size="12"/></button></div><button class="new-tab" title="打开连接" @click="manager=true"><Plus :size="16"/></button><span class="flex-spacer"/><button class="icon-button" title="重新连接" :disabled="!active||active.status==='connecting'" @click="run(reconnect)"><RefreshCw :size="14"/></button><button class="icon-button" title="断开连接" :disabled="active?.status!=='connected'" @click="run(disconnectActive)"><Unplug :size="14"/></button></div>
  <main class="workspace" ref="workspace"><MonitorPanel :tab="active"/><div class="session-workspace" :style="{'--file-height':fileHeight+'px'}">
    <section class="terminal-pane"><div class="terminal-toolbar"><span><Terminal :size="13"/> {{ active ? active.connection.username+'@'+active.connection.host : 'SSH 终端' }}</span><span class="flex-spacer"/><button class="icon-button" title="终端搜索" @click="searchVisible=!searchVisible"><Search :size="14"/></button><button class="font-button" title="减小字号" @click="fontSize=Math.max(10,fontSize-1)">A−</button><button class="font-button" title="增大字号" @click="fontSize=Math.min(24,fontSize+1)">A+</button><button class="icon-button" title="切换终端背景" @click="terminalLight=!terminalLight"><Sun v-if="terminalLight" :size="14"/><Moon v-else :size="14"/></button></div>
      <div v-if="searchVisible" class="terminal-search"><input v-model="searchText" placeholder="搜索终端文本" aria-label="搜索终端" @keyup.enter="active&&terminalRefs.get(active.id)?.find(searchText)"/><button @click="active&&terminalRefs.get(active.id)?.find(searchText)">查找</button><button class="icon-button" @click="searchVisible=false"><X :size="13"/></button></div>
      <div v-show="activeId==='welcome'" class="welcome"><div class="welcome-icon"><Terminal :size="37"/></div><h1>连接你的服务器</h1><p>终端、文件与系统监控，同一个工作区。</p><div class="welcome-buttons"><button class="primary" @click="createConnection"><Plus :size="16"/> 新建 SSH 连接</button><button @click="run(()=>doImport(false))"><Download :size="16"/> 导入 FinalShell 连接</button></div><div v-if="connections.length" class="recent-connections"><div class="recent-heading">已保存的连接 <button @click="manager=true">全部连接 <ArrowRight :size="12"/></button></div><button v-for="c in connections.slice(0,6)" :key="c.id" @click="run(()=>openConnection(c))"><Monitor :size="17"/><div><strong>{{ c.name }}</strong><small>{{ c.username }}@{{ c.host }}:{{ c.port }}</small></div><ArrowRight :size="14"/></button></div><small v-else class="welcome-hint">支持 *_connect_config.json 和 conn 目录导入</small></div>
      <TerminalView v-for="t in tabs" v-show="activeId===t.id" :key="t.id" :ref="value=>terminalRef(t.id,value)" :active="activeId===t.id" :connected="t.status==='connected'" :font-size="fontSize" :light="terminalLight" @input="value=>sendInput(t.id,value)" @resize="(cols,rows)=>resizeTerminal(t.id,cols,rows)" @consumed="count=>consumed(t.id,count)" @directory="path=>onDirectory(t.id,path)"/>
    </section><div class="splitter" role="separator" aria-label="调整终端与文件区高度" @pointerdown="startResize"/><FilePanel ref="fileRef" :tab="active" :transfer-count="runningTransfers" @path="path=>{if(active)active.path=path;}" @editor="file=>run(()=>openEditor(file))" @action="(action,file)=>run(()=>fileAction(action,file))" @upload="run(uploadFiles)" @download="file=>run(()=>downloadFile(file))" @transfers="showTransfers=true"/>
    <form class="command-bar" @submit.prevent="run(executeCommand)"><Terminal :size="16"/><input v-model="command" list="command-history" placeholder="命令输入框：输入命令，按 Enter 发送" aria-label="命令输入框"/><datalist id="command-history"><option v-for="item in history.slice(0,80)" :key="item" :value="item"/></datalist><button class="command-send" type="submit" :disabled="active?.status!=='connected'">发送 <ArrowRight :size="14"/></button><button class="icon-button" type="button" title="历史命令" @click="command=history[0]||''"><ChevronDown :size="15"/></button></form>
    <div class="quick-bar"><span>快捷命令</span><button v-for="c in quickCommands" :key="c" :title="'点击填入；双击发送 '+c" @click="command=c" @dblclick="run(executeCommand)">{{ c }}</button><button class="icon-button" title="编辑快捷命令" @click="quickText=quickCommands.join('\n');quickEditor=true"><Pencil :size="13"/></button><span class="flex-spacer"/><label class="checkbox"><input v-model="followDirectory" type="checkbox"/> 目录联动</label></div>
  </div></main>
  <footer class="statusbar"><span class="status-dot" :class="{online:active?.status==='connected'}"/>{{ statusName }}<span v-if="active">{{ active.connection.host }}:{{ active.connection.port }}</span><span class="flex-spacer"/><button @click="showTransfers=true">{{ runningTransfers ? runningTransfers+' 个传输任务' : '传输队列' }}</button><span>UTF-8</span><span>ShellDock 0.1.0</span></footer>
  <div v-if="notice" class="toast" :class="{error:noticeError}" role="status"><Info v-if="noticeError" :size="17"/><Check v-else :size="17"/><span>{{ notice }}</span><button class="icon-button" aria-label="关闭提示" @click="notice='' "><X :size="15"/></button></div>
  <ConnectionManager v-if="manager" :connections="connections" @close="manager=false" @create="createConnection" @edit="c=>form={connection:c}" @connect="c=>run(()=>openConnection(c))" @remove="c=>run(()=>removeConnection(c))" @import="directory=>run(()=>doImport(directory))" @export="prepareExport"/>
  <ConnectionForm v-if="form" :connection="form.connection" @close="form=null" @saved="onSaved"/>
  <RemoteEditor v-if="editor" :session-id="editor.sessionId" :text="editor.text" @close="dirty=>run(()=>closeEditor(dirty))" @saved="fileRef?.refresh()"/>
  <div v-if="promptDialog" class="overlay"><form class="dialog prompt-dialog" role="dialog" aria-modal="true" :aria-label="promptDialog.title" @submit.prevent="answerPrompt(promptDialog.value)"><div class="dialog-title"><span>{{ promptDialog.title }}</span><button class="icon-button" type="button" @click="answerPrompt(null)"><X :size="17"/></button></div><div class="form-content"><p v-if="promptDialog.help" class="muted">{{ promptDialog.help }}</p><input v-model="promptDialog.value" :type="promptDialog.type" :aria-label="promptDialog.title" autofocus autocomplete="off"/></div><div class="dialog-footer"><button type="button" @click="answerPrompt(null)">取消</button><button class="primary" type="submit">确定</button></div></form></div>
  <div v-if="exportDialog" class="overlay"><section class="dialog prompt-dialog" role="dialog" aria-modal="true" aria-label="导出连接"><div class="dialog-title"><span>导出连接</span><button class="icon-button" @click="exportDialog=null"><X :size="17"/></button></div><div class="form-content"><p>{{ exportDialog.connection ? exportDialog.connection.name : '全部 '+connections.length+' 个连接' }}</p><label>导出格式<select v-model="exportDialog.format"><option value="shelldock">ShellDock JSON（分组与连接，不含凭据）</option><option v-if="exportDialog.connection" value="finalshell">FinalShell 单连接 JSON</option></select></label><label v-if="exportDialog.format==='finalshell'" class="checkbox"><input v-model="exportDialog.includePasswords" type="checkbox"/> 包含已保存的密码（FinalShell 兼容格式）</label><p class="muted">代理和转发未知字段会保留；私钥文件需在目标电脑重新选择。</p></div><div class="dialog-footer"><button @click="exportDialog=null">取消</button><button class="primary" @click="run(doExport)"><Upload :size="15"/> 导出</button></div></section></div>
  <div v-if="showTransfers" class="overlay" @mousedown.self="showTransfers=false"><section class="dialog transfer-dialog" role="dialog" aria-modal="true" aria-label="文件传输"><div class="dialog-title"><span><Upload :size="16"/> 文件传输</span><button class="icon-button" @click="showTransfers=false"><X :size="18"/></button></div><div class="transfer-list"><div v-for="t in transfers" :key="t.id" class="transfer-row"><Download v-if="t.direction==='download'" :size="20"/><Upload v-else :size="20"/><div class="grow"><strong>{{ t.name }}</strong><small>{{ bytes(t.transferred) }} / {{ bytes(t.total) }} · {{ {queued:'等待中',running:'传输中',completed:'已完成',failed:'失败',cancelled:'已取消'}[t.status as 'queued'] || t.status }}</small><div class="meter"><i :style="{width:(t.total ? Math.min(100,t.transferred/t.total*100):0)+'%'}"/></div><p v-if="t.error" class="form-error">{{ t.error }}</p></div><button v-if="['queued','running'].includes(t.status)" @click="run(()=>call('cancel_transfer',{id:t.id}))">取消</button><Check v-else-if="t.status==='completed'" :size="17" class="success"/></div><div v-if="!transfers.length" class="manager-empty"><Upload :size="30"/><p>没有传输任务</p></div></div><div class="dialog-footer"><button @click="transfers=transfers.filter(t=>['queued','running'].includes(t.status))">清除已结束</button><button @click="showTransfers=false">关闭</button></div></section></div>
  <div v-if="quickEditor" class="overlay"><section class="dialog prompt-dialog" role="dialog" aria-modal="true" aria-label="编辑快捷命令"><div class="dialog-title"><span>编辑快捷命令</span><button class="icon-button" @click="quickEditor=false"><X :size="17"/></button></div><div class="form-content"><p class="muted">每行一个命令。点击填入命令框，双击发送。</p><textarea v-model="quickText" rows="10" aria-label="快捷命令列表"/></div><div class="dialog-footer"><button @click="quickEditor=false">取消</button><button class="primary" @click="run(saveQuickCommands)">保存</button></div></section></div>
  <div v-if="about" class="overlay" @mousedown.self="about=false"><section class="dialog prompt-dialog" role="dialog" aria-modal="true" aria-label="关于 ShellDock"><div class="dialog-title"><span>关于 ShellDock</span><button class="icon-button" @click="about=false"><X :size="17"/></button></div><div class="form-content"><h2>ShellDock</h2><p>SSH 终端、SFTP 与 Linux 系统监控。</p><p class="muted">首次版本支持 FinalShell 连接 JSON / conn 目录导入。目录联动需要远端 shell 发送 OSC 7；未配置时可手动输入路径。</p><p class="muted">RDP、代理执行、端口转发和云同步尚未实现。</p><p v-if="!desktop" class="warning">当前是浏览器界面预览，不能执行 SSH、文件或监控操作。</p></div><div class="dialog-footer"><button @click="about=false">关闭</button></div></section></div>
</div></template>
