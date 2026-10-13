<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue';
import { X, Save, FileText } from 'lucide-vue-next';
import { EditorView, basicSetup } from 'codemirror';
import { StreamLanguage } from '@codemirror/language';
import { keymap } from '@codemirror/view';
import { shell } from '@codemirror/legacy-modes/mode/shell';
import { javascript } from '@codemirror/lang-javascript';
import { json } from '@codemirror/lang-json';
import type { RemoteText, Revision } from '../lib/types';
import { call } from '../lib/api';
const props=defineProps<{sessionId:string;text:RemoteText}>();
const emit=defineEmits<{close:[dirty:boolean];saved:[]}>();
const element=ref<HTMLDivElement>();let view:EditorView;let revision=props.text.revision;
const dirty=ref(false);const saving=ref(false);const error=ref('');
async function save(){saving.value=true;error.value='';try{revision=await call<Revision>('save_remote_text',{sessionId:props.sessionId,text:{path:props.text.path,content:view.state.doc.toString(),revision}});dirty.value=false;emit('saved');}catch(e){error.value=String(e);}finally{saving.value=false;}}
onMounted(()=>{const name=props.text.path.toLowerCase();const language=name.endsWith('.json')?json():/\.(js|ts|jsx|tsx)$/.test(name)?javascript({typescript:name.endsWith('.ts')}):StreamLanguage.define(shell);view=new EditorView({doc:props.text.content,parent:element.value!,extensions:[basicSetup,language,keymap.of([{key:'Mod-s',run:()=>{void save();return true;}}]),EditorView.updateListener.of(update=>{if(update.docChanged)dirty.value=true;}),EditorView.theme({'&':{height:'100%'},'.cm-scroller':{overflow:'auto',fontFamily:'Consolas, monospace',fontSize:'13px'}})]});});
onBeforeUnmount(()=>view?.destroy());
</script>
<template><div class="overlay"><section class="dialog editor-dialog" role="dialog" aria-modal="true" aria-label="远端文本编辑器"><div class="dialog-title"><span><FileText :size="16"/>{{ text.path }}{{ dirty?' *':'' }}</span><button class="icon-button" aria-label="关闭编辑器" @click="emit('close',dirty)"><X :size="18"/></button></div><div ref="element" class="code-editor"/><p v-if="error" class="form-error" role="alert">{{ error }}</p><div class="dialog-footer"><span class="muted">UTF-8 · Ctrl+S 保存到服务器</span><button @click="emit('close',dirty)">关闭</button><button class="primary" :disabled="saving" @click="save"><Save :size="15"/>{{ saving?'保存中…':'保存' }}</button></div></section></div></template>
