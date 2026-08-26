<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{ url: string; name: string }>()
const img = ref<HTMLImageElement>()
const err = ref('')

watch(
  () => props.url,
  (u) => {
    err.value = ''
    if (img.value) {
      img.value.onerror = () => (err.value = '图片加载失败')
      img.value.src = u
    }
  },
  { immediate: true },
)

function preventDrag(e: DragEvent) {
  e.preventDefault()
}
</script>

<template>
  <div class="img-wrap" @dragstart.prevent @drop.prevent @dragover.prevent>
    <img
      ref="img"
      :src="url"
      :alt="name"
      class="img-view"
      draggable="false"
      @dragstart="preventDrag"
      @drag="preventDrag"
    />
    <div v-if="err" class="img-error">{{ err }}</div>
  </div>
</template>

<style scoped>
.img-wrap {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
}
.img-view {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  user-select: none;
}
.img-error {
  color: var(--danger);
}
</style>
