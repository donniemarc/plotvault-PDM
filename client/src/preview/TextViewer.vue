<script setup lang="ts">
defineProps<{ text: string }>()
</script>

<template>
  <div class="text-wrap">
    <pre class="text-view">{{ text }}</pre>
  </div>
</template>

<style scoped>
.text-wrap {
  width: 100%;
  height: 100%;
  overflow: auto;
  padding: 16px;
}
.text-view {
  margin: 0;
  font-family: Consolas, 'Courier New', monospace;
  font-size: 12px;
  line-height: 1.5;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-all;
}
</style>
