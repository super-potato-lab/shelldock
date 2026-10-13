<script setup lang="ts">
import { ref, computed } from 'vue';
import { Activity, Cpu, Network, HardDrive, ChevronDown, Server } from 'lucide-vue-next';
import type { Tab } from '../lib/types';
import { bytes, percent } from '../lib/format';
const props = defineProps<{tab?: Tab}>();
const nic = ref('');
const m = computed(() => props.tab?.metrics);
const mem = computed(() => m.value?.memoryTotal ? (1 - m.value.memoryAvailable / m.value.memoryTotal) * 100 : null);
const activeNic = computed(() => m.value?.interfaces.find(n => n.name === nic.value) || m.value?.interfaces.find(n => n.name !== 'lo'));
</script>
<template>
  <aside class="monitor-panel">
    <div class="monitor-title"><Server :size="15"/> 系统监控 <span class="status-dot" :class="{online: tab?.status === 'connected'}"/></div>
    <div class="host-meta"><strong>{{ tab?.connection.host || '未连接主机' }}</strong><span>{{ m?.system || 'SSH / Linux' }}</span><span>{{ tab?.status === 'connected' ? '运行 ' + Math.floor((m?.uptime || 0) / 3600) + ' 小时' : '连接后自动采集系统信息' }}</span></div>
    <p v-if="tab?.metricError" class="metric-error" role="status">{{ tab.metricError }}</p>
    <div class="metric-section"><div class="section-heading"><Cpu :size="14"/> CPU <b>{{ percent(m?.cpuPercent) }}</b></div><div class="meter"><i :style="{width: (m?.cpuPercent || 0) + '%'}"/></div><div class="spark-placeholder"><svg viewBox="0 0 200 35"><path d="M0 32H200"/></svg><span>{{ m?.cpuPercent == null ? '等待两次采样' : '实时 CPU 利用率' }}</span></div></div>
    <div class="metric-section"><div class="section-heading"><Activity :size="14"/> 内存 <b>{{ percent(mem) }}</b></div><div class="meter memory"><i :style="{width: (mem || 0) + '%'}"/></div><small>{{ m ? bytes(m.memoryTotal - m.memoryAvailable) + ' / ' + bytes(m.memoryTotal) : '— / —' }}</small><div class="muted-row"><span>交换分区</span><span>{{ m ? bytes(m.swapTotal - m.swapFree) + ' / ' + bytes(m.swapTotal) : '—' }}</span></div><div class="muted-row"><span>系统负载</span><span>{{ m?.load.map(n => n.toFixed(2)).join(' / ') || '—' }}</span></div></div>
    <div class="metric-section"><div class="section-heading"><Network :size="14"/> 网络 <select v-model="nic" aria-label="监控网卡"><option value="">自动选择</option><option v-for="n in m?.interfaces" :key="n.name" :value="n.name">{{ n.name }}</option></select></div><div class="rate-row"><span class="receive">↓ {{ bytes(activeNic?.receiveRate) }}/s</span><span class="send">↑ {{ bytes(activeNic?.sendRate) }}/s</span></div><small>接收 {{ bytes(activeNic?.received) }}　发送 {{ bytes(activeNic?.sent) }}</small></div>
    <div class="metric-section disks"><div class="section-heading"><HardDrive :size="14"/> 磁盘 <ChevronDown :size="13"/></div><div v-for="disk in m?.disks.filter(d => d.name.startsWith('/dev/')).slice(0, 4)" :key="disk.mount" class="disk"><div><span>{{ disk.mount }}</span><b>{{ percent(disk.total ? disk.used / disk.total * 100 : 0) }}</b></div><div class="meter disk-meter"><i :style="{width: (disk.total ? disk.used / disk.total * 100 : 0) + '%'}"/></div><small>{{ bytes(disk.used) }} / {{ bytes(disk.total) }}</small></div><p v-if="!m" class="empty-small">连接后显示挂载点和使用率</p></div>
    <div class="process-section"><div class="section-heading">进程 <span class="muted">CPU / 内存</span></div><table><thead><tr><th>名称</th><th>CPU</th><th>内存</th></tr></thead><tbody><tr v-for="p in m?.processes.slice(0, 8)" :key="p.pid" :title="`${p.user} · PID ${p.pid}`"><td>{{ p.command }}</td><td>{{ p.cpu.toFixed(1) }}</td><td>{{ p.memory.toFixed(1) }}</td></tr></tbody></table><p v-if="!m?.processes.length" class="empty-small">暂无进程信息</p></div>
    <div class="monitor-footer">{{ m ? '最近更新 ' + new Date(m.timestamp * 1000).toLocaleTimeString('zh-CN', {hour12:false}) : '监控数据通过 SSH 采集' }}</div>
  </aside>
</template>
