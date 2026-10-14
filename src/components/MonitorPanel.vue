<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Tab, Metrics } from '../lib/types';
import { compactBytes } from '../lib/format';
const props = defineProps<{tab?: Tab; quickCommands: string[]}>();
const emit = defineEmits<{system: []; copy: []; command: [value: string]; editCommands: []}>();
const nic = ref(''); const chart = ref<'memory'|'cpu'|'commands'>('cpu');
const m = computed(() => props.tab?.metrics);
const history = computed(() => props.tab?.metricHistory || []);
const memory = (sample: Metrics) => sample.memoryTotal ? (1 - sample.memoryAvailable / sample.memoryTotal) * 100 : null;
const mem = computed(() => m.value ? memory(m.value) : null);
const swap = computed(() => m.value ? (m.value.swapTotal ? (1 - m.value.swapFree/m.value.swapTotal)*100 : 0) : null);
const activeNic = computed(() => m.value?.interfaces.find(n => n.name === nic.value) || m.value?.interfaces.find(n => n.name !== 'lo') || m.value?.interfaces[0]);
function integerPercent(n: number | null | undefined) { return n == null ? '—' : Math.round(n)+'%'; }
function width(n: number | null | undefined) { return Math.max(0, Math.min(100, n || 0))+'%'; }
function points(values: (number | null | undefined)[], max: number, height: number) {
  return values.map((n, i) => n == null || !Number.isFinite(n) ? null : `${300-(values.length-1-i)*2.5},${height-3-Math.min(max,Math.max(0,n))/Math.max(1,max)*(height-6)}`).filter(Boolean).join(' ');
}
const usagePoints = computed(() => points(history.value.map(s => chart.value === 'memory' ? memory(s) : s.cpuPercent), 100, 80));
const rates = computed(() => history.value.map(s => s.interfaces.find(n => n.name === activeNic.value?.name)));
const networkMax = computed(() => Math.max(9, ...rates.value.flatMap(n => [n?.receiveRate || 0, n?.sendRate || 0])));
const receivePoints = computed(() => points(rates.value.map(n => n?.receiveRate), networkMax.value, 56));
const sendPoints = computed(() => points(rates.value.map(n => n?.sendRate), networkMax.value, 56));
</script>
<template>
  <aside class="monitor-panel" aria-label="系统监控">
    <div class="sync-state" title="云同步尚未实现">同步状态 <span class="status-dot"/></div>
    <div class="monitor-ip"><span>IP {{ tab?.connection.host || '-' }}</span><button :disabled="!tab" @click="emit('copy')">复制</button></div>
    <button class="system-info" @click="emit('system')">系统信息</button>
    <div class="monitor-line">运行 {{ m ? Math.floor(m.uptime/86400)+' 天' : '-' }}</div>
    <div class="monitor-line">负载 {{ m?.load.map(n=>n.toFixed(2)).join(', ') || '-' }}</div>
    <div class="usage-row"><span>CPU</span><div class="usage-bar"><i :style="{width:width(m?.cpuPercent)}"/><span>{{ integerPercent(m?.cpuPercent) }}</span></div></div>
    <div class="usage-row"><span>内存</span><div class="usage-bar"><i :style="{width:width(mem)}"/><span>{{ integerPercent(mem) }}</span><span>{{ m ? compactBytes(m.memoryTotal-m.memoryAvailable)+'/'+compactBytes(m.memoryTotal) : '—/—' }}</span></div></div>
    <div class="usage-row"><span>交换</span><div class="usage-bar"><i :style="{width:width(swap)}"/><span>{{ integerPercent(swap) }}</span><span>{{ m ? compactBytes(m.swapTotal-m.swapFree)+'/'+compactBytes(m.swapTotal) : '—/—' }}</span></div></div>
    <div class="usage-chart">
      <div class="monitor-tabs"><button :class="{active:chart==='memory'}" @click="chart='memory'">内存</button><button :class="{active:chart==='cpu'}" @click="chart='cpu'">CPU</button><button :class="{active:chart==='commands'}" @click="chart='commands'">命令</button></div>
      <div v-if="chart==='commands'" class="monitor-commands"><button v-for="value in quickCommands" :key="value" @click="emit('command',value)">{{ value }}</button><button class="edit-commands" @click="emit('editCommands')">编辑…</button></div>
      <svg v-else viewBox="0 0 300 80" preserveAspectRatio="none" role="img" :aria-label="chart==='cpu'?'CPU 历史曲线':'内存历史曲线'"><polyline :points="usagePoints"/></svg>
    </div>
    <p v-if="tab?.metricError" class="metric-error" role="status">{{ tab.metricError }}</p>
    <div class="network-heading"><span class="send">↑{{ activeNic?.sendRate==null?'':compactBytes(activeNic.sendRate) }}</span><span class="receive">↓{{ activeNic?.receiveRate==null?'':compactBytes(activeNic.receiveRate) }}</span><select v-model="nic" aria-label="监控网卡"><option value="">{{ activeNic?.name || '' }}</option><option v-for="n in m?.interfaces" :key="n.name" :value="n.name">{{ n.name }}</option></select></div>
    <div class="network-chart"><div class="chart-labels"><span>{{ compactBytes(networkMax) }}</span><span>{{ compactBytes(networkMax*2/3) }}</span><span>{{ compactBytes(networkMax/3) }}</span></div><svg viewBox="0 0 300 56" preserveAspectRatio="none" role="img" aria-label="网络速率历史曲线"><path class="grid" d="M0 3H300 M0 21H300 M0 39H300"/><polyline class="receive-line" :points="receivePoints"/><polyline class="send-line" :points="sendPoints"/></svg></div>
    <div class="latency-heading" title="当前版本尚未实现延迟测量"><span>— ms</span><span>本机</span></div>
    <div class="network-chart latency-chart" title="延迟测量尚未实现"><div class="chart-labels"><span>—</span><span>—</span><span>—</span></div><svg viewBox="0 0 300 56" preserveAspectRatio="none" aria-hidden="true"><path class="grid" d="M0 3H300 M0 21H300 M0 39H300"/></svg></div>
    <div class="disks-table-wrap"><table class="disks-table"><thead><tr><th>路径</th><th>可用/大小</th></tr></thead><tbody><tr v-for="disk in m?.disks" :key="disk.mount" :title="disk.name"><td>{{ disk.mount }}</td><td>{{ compactBytes(disk.available) }}/{{ compactBytes(disk.total) }}</td></tr></tbody></table></div>
  </aside>
</template>
