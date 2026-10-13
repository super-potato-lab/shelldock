<script setup lang="ts">
import { reactive, ref } from 'vue';
import { X, FolderOpen, Monitor, ShieldCheck } from 'lucide-vue-next';
import { open } from '@tauri-apps/plugin-dialog';
import type { Connection } from '../lib/types';
import { call, desktop } from '../lib/api';
const props = defineProps<{connection?: Connection}>();
const emit = defineEmits<{close: []; saved: [connection: Connection]}>();
const form = reactive<Connection>(props.connection ? JSON.parse(JSON.stringify(props.connection)) : {id:'',name:'',host:'',port:22,username:'root',group:'',auth:'password',privateKeyPath:null,notes:'',hasPassword:false,protocol:'ssh',extra:{}});
const password=ref('');const remember=ref(true);const saving=ref(false);const error=ref('');const direct=ref(false);
const proxyRef=String(form.extra?.proxy_id || '');
async function chooseKey(){if(!desktop){error.value='请在桌面应用中选择私钥文件';return;}const p=await open({multiple:false,directory:false,title:'选择私钥文件'});if(typeof p==='string')form.privateKeyPath=p;}
async function save(){saving.value=true;error.value='';try{const c=JSON.parse(JSON.stringify(form));if(direct.value&&c.extra){c.extra.proxy_id='0';}emit('saved',await call<Connection>('save_connection',{connection:c,password:password.value||null,remember:remember.value}));password.value='';}catch(e){error.value=String(e);}finally{saving.value=false;}}
</script>
<template><div class="overlay" @mousedown.self="emit('close')"><form class="dialog connection-form" role="dialog" aria-modal="true" aria-label="SSH 连接设置" @submit.prevent="save">
<div class="dialog-title"><span><Monitor :size="17"/> {{ connection ? '编辑 SSH 连接' : '新建 SSH 连接' }}</span><button type="button" class="icon-button" aria-label="关闭连接设置" @click="emit('close')"><X :size="18"/></button></div>
<div class="form-content"><div class="form-section-title">基本信息</div><label>名称<input v-model="form.name" required placeholder="例如：生产服务器" autofocus/></label><div class="form-row"><label class="grow">主机地址<input v-model="form.host" required placeholder="IP 或域名"/></label><label class="port-field">端口<input v-model.number="form.port" type="number" min="1" max="65535" required/></label></div><label>分组<input v-model="form.group" placeholder="例如：生产环境 / 北京"/></label><div class="form-section-title"><ShieldCheck :size="14"/> 身份认证</div><div class="form-row"><label class="grow">用户名<input v-model="form.username" required/></label><label class="grow">认证方式<select v-model="form.auth"><option value="password">密码</option><option value="key">私钥文件</option></select></label></div><label v-if="form.auth==='key'">私钥文件<div class="input-with-button"><input v-model="form.privateKeyPath" required placeholder="选择本机私钥文件"/><button type="button" aria-label="选择私钥文件" @click="chooseKey"><FolderOpen :size="16"/></button></div></label><label>{{ form.auth==='key'?'私钥口令（可为空）':'密码' }}<input v-model="password" type="password" autocomplete="new-password" :placeholder="form.hasPassword?'已保存，留空保持原值':'连接时也可以输入'"/></label><label class="checkbox"><input v-model="remember" type="checkbox"/> 使用系统凭据服务保存密码 / 口令</label><label>备注<textarea v-model="form.notes" rows="2"/></label><label v-if="proxyRef && proxyRef!=='0'" class="checkbox warning"><input v-model="direct" type="checkbox"/> 清除原代理引用 {{ proxyRef }}，使用直接连接</label><p v-if="error" class="form-error" role="alert">{{ error }}</p></div>
<div class="dialog-footer"><button type="button" @click="emit('close')">取消</button><button class="primary" type="submit" :disabled="saving">{{ saving?'保存中…':'保存' }}</button></div></form></div></template>
