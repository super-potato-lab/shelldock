<script setup lang="ts">
import { ref, watch, computed, onBeforeUnmount } from 'vue';
import { ArrowUp, RefreshCw, Upload, Download, FolderPlus, Pencil, Trash2, Lock, ChevronDown } from 'lucide-vue-next';
import RemoteTree from './RemoteTree.vue';
import type { Tab, FileEntry } from '../lib/types';
import { call } from '../lib/api';
import { bytes, remoteTime, parentRemote, permissionsText } from '../lib/format';
const props=defineProps<{tab?: Tab; transferCount:number; quickCommands: string[]}>();
const emit=defineEmits<{path:[path:string];editor:[file:FileEntry];action:[action:string,file?:FileEntry];upload:[];download:[file:FileEntry];transfers:[];command:[value:string];editCommands:[]}>();
const files=ref<FileEntry[]>([]); const selected=ref<string>(); const pathInput=ref('/'); const error=ref(''); const loading=ref(false);
const panelTab=ref('files'); const pathHistory=ref<string[]>([]); const historyVisible=ref(false); const tree=ref<InstanceType<typeof RemoteTree>>();
const sort=ref<'name'|'size'|'modified'>('name'); const ascending=ref(true); let generation=0;
const current=computed(()=>files.value.find(f=>f.path===selected.value));
const visible=computed(()=>[...files.value].sort((a,b)=> Number(b.isDir)-Number(a.isDir) || (sort.value==='name'?a.name.localeCompare(b.name):a[sort.value]-b[sort.value])*(ascending.value?1:-1)));
async function readDirectory(){const run=++generation;files.value=[];selected.value=undefined;error.value='';if(props.tab?.status!=='connected'||!props.tab.sessionId){loading.value=false;return;}loading.value=true;try{const result=await call<FileEntry[]>('list_files',{sessionId:props.tab.sessionId,path:props.tab.path});if(run===generation)files.value=result;}catch(e){if(run===generation)error.value=String(e);}finally{if(run===generation)loading.value=false;}}
async function refresh(){await Promise.all([readDirectory(),tree.value?.refresh()]);}
function navigate(){const path=pathInput.value.trim();if(path.startsWith('/'))emit('path',path);else error.value='请输入以 / 开头的绝对路径';}
function activate(f:FileEntry){if(f.isDir)emit('path',f.path);else emit('editor',f);}
function setSort(column:'name'|'size'|'modified'){if(sort.value===column)ascending.value=!ascending.value;else{sort.value=column;ascending.value=true;}}
function fileType(f:FileEntry){if(f.isDir)return '文件夹';if(f.isSymlink)return '符号链接';const ext=f.name.includes('.')?f.name.split('.').at(-1):'';return ext?ext.toUpperCase()+' 文件':'文件';}
watch(()=>props.tab?.id,()=>{pathHistory.value=[];historyVisible.value=false;});
watch(()=>[props.tab?.id,props.tab?.path,props.tab?.status],()=>{pathInput.value=props.tab?.path||'/';if(props.tab?.status==='connected')pathHistory.value=[pathInput.value,...pathHistory.value.filter(p=>p!==pathInput.value)].slice(0,30);void readDirectory();},{immediate:true});
onBeforeUnmount(()=>generation++);
defineExpose({refresh});
</script>
<template>
<section class="file-panel">
  <div class="file-tabs"><button :class="{active:panelTab==='files'}" @click="panelTab='files'">文件</button><button :class="{active:panelTab==='commands'}" @click="panelTab='commands'">命令</button><span class="flex-spacer"/><button v-if="transferCount" class="transfer-count" @click="emit('transfers')">传输 {{ transferCount }}</button></div>
  <template v-if="panelTab==='files'">
    <div class="file-toolbar">
      <form class="path-field" @submit.prevent="navigate"><input v-model="pathInput" aria-label="远端路径" :disabled="tab?.status!=='connected'"/></form>
      <button class="native-mini" @click="historyVisible=!historyVisible">历史</button>
      <button class="icon-button" aria-label="刷新文件" title="刷新文件" :disabled="loading || tab?.status!=='connected'" @click="refresh"><RefreshCw :size="15" :class="{spinning:loading}"/></button>
      <button class="icon-button" title="上一级" :disabled="tab?.status!=='connected'" @click="emit('path',parentRemote(tab?.path||'/'))"><ArrowUp :size="16"/></button>
      <button class="icon-button" aria-label="下载文件" title="下载文件" :disabled="!current||current.isDir" @click="current&&emit('download',current)"><Download :size="16"/></button><button class="icon-button" aria-label="上传文件" title="上传文件" :disabled="tab?.status!=='connected'" @click="emit('upload')"><Upload :size="16"/></button>
      <span class="flex-spacer"/><button class="icon-button" title="新建文件夹" :disabled="tab?.status!=='connected'" @click="emit('action','mkdir')"><FolderPlus :size="16"/></button><button class="icon-button" title="重命名" :disabled="!current" @click="emit('action','rename',current)"><Pencil :size="14"/></button><button class="icon-button" title="权限" :disabled="!current" @click="emit('action','chmod',current)"><Lock :size="14"/></button><button class="icon-button" title="删除" :disabled="!current" @click="emit('action','delete',current)"><Trash2 :size="14"/></button><button class="icon-button" title="文件传输" @click="emit('transfers')"><ChevronDown :size="14"/></button>
      <div v-if="historyVisible" class="path-history"><button v-for="path in pathHistory" :key="path" @click="emit('path',path);historyVisible=false">{{ path }}</button><span v-if="!pathHistory.length">暂无目录历史</span></div>
    </div>
    <div class="file-browser">
      <RemoteTree ref="tree" :session-id="tab?.status==='connected'?tab.sessionId:undefined" :path="tab?.path||'/'" @navigate="path=>emit('path',path)"/>
      <div class="file-table-wrap"><table class="file-table"><colgroup><col style="width:160px"/><col style="width:76px"/><col style="width:80px"/><col style="width:135px"/><col style="width:100px"/><col style="width:110px"/><col/></colgroup><thead><tr><th><button @click="setSort('name')">文件名 <span v-if="sort==='name'">{{ ascending?'⌃':'⌄' }}</span></button></th><th><button @click="setSort('size')">大小</button></th><th>类型</th><th><button @click="setSort('modified')">修改时间</button></th><th>权限</th><th>用户/用户组</th><th/></tr></thead><tbody><tr v-for="f in visible" :key="f.path" :class="{selected:selected===f.path}" @click="selected=f.path" @dblclick="activate(f)"><td><span :class="f.isDir?'folder-glyph':'file-glyph'"/>{{ f.name }}</td><td>{{ f.isDir?'':bytes(f.size) }}</td><td>{{ fileType(f) }}</td><td>{{ remoteTime(f.modified) }}</td><td>{{ permissionsText(f.permissions,f.isDir,f.isSymlink) }}</td><td>{{ f.uid==null?'—':f.uid }}/{{ f.gid==null?'—':f.gid }}</td><td/></tr></tbody></table><div v-if="!visible.length" class="file-empty">{{ loading?'正在读取目录…':error|| (tab?.status==='connected'?'目录为空':'连接服务器后浏览远端文件') }}</div></div>
    </div>
  </template>
  <div v-else class="file-commands"><div><button @click="emit('editCommands')">编辑快捷命令</button><span>点击命令填入输入框，按 Enter 发送。</span></div><button v-for="value in quickCommands" :key="value" @click="emit('command',value)">{{ value }}</button></div>
</section>
</template>
