<script setup lang="ts">
import { computed } from 'vue'
import { Pie, Bar } from 'vue-chartjs'
import {
  Chart as ChartJS,
  ArcElement,
  Tooltip,
  Legend,
  CategoryScale,
  LinearScale,
  BarElement,
} from 'chart.js'

ChartJS.register(ArcElement, Tooltip, Legend, CategoryScale, LinearScale, BarElement)

const props = defineProps<{
  files: { ext: string }[]
  folders: { name: string }[]
}>()

// 文件类型统计
const typeStats = computed(() => {
  const map = new Map<string, number>()
  props.files.forEach(f => {
    const ext = (f.ext || '其他').toUpperCase()
    map.set(ext, (map.get(ext) || 0) + 1)
  })
  return Array.from(map.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 6)
})

// 饼图数据
const pieData = computed(() => ({
  labels: typeStats.value.map(t => t[0]),
  datasets: [{
    data: typeStats.value.map(t => t[1]),
    backgroundColor: [
      '#6366f1', '#22c55e', '#f59e0b', '#ef4444', '#3b82f6', '#8b5cf6'
    ],
    borderWidth: 0,
  }]
}))

const pieOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      position: 'right' as const,
      labels: { boxWidth: 12, padding: 12, font: { size: 12 } }
    }
  }
}

// 柱状图数据：按首字母分组的文件数量
const barData = computed(() => {
  const groups: Record<string, number> = {}
  props.files.forEach(f => {
    const first = (f.ext || '?')[0].toUpperCase()
    const bucket = first.match(/[0-9]/) ? '#数字' : first.match(/[A-H]/i) ? 'A-H' : first.match(/[I-P]/i) ? 'I-P' : 'Q-Z'
    groups[bucket] = (groups[bucket] || 0) + 1
  })
  const ordered = ['#数字', 'A-H', 'I-P', 'Q-Z'].map(k => [k, groups[k] || 0] as [string, number])
  return {
    labels: ordered.map(t => t[0]),
    datasets: [{
      label: '文件数量',
      data: ordered.map(t => t[1]),
      backgroundColor: '#6366f1',
      borderRadius: 4,
    }]
  }
})

const barOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: { legend: { display: false } },
  scales: {
    y: { beginAtZero: true, ticks: { stepSize: 1, font: { size: 11 } } },
    x: { ticks: { font: { size: 11 } } }
  }
}
</script>

<template>
  <div class="dashboard-charts">
    <div class="chart-card" v-if="files.length > 0">
      <h4>文件类型分布</h4>
      <div class="chart-box">
        <Pie :data="pieData" :options="pieOptions" />
      </div>
    </div>
    <div class="chart-card" v-if="files.length > 0">
      <h4>文件按类型分组</h4>
      <div class="chart-box">
        <Bar :data="barData" :options="barOptions" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard-charts {
  display: flex;
  gap: 16px;
  width: 100%;
  max-width: 600px;
}
.chart-card {
  flex: 1;
  background: var(--bg-panel);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 16px;
  min-height: 180px;
}
.chart-card h4 {
  margin: 0 0 8px;
  font-size: var(--font-sm);
  color: var(--text-dim);
  text-align: center;
}
.chart-box {
  height: 160px;
}
</style>
