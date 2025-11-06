<template>
  <div v-if="items.length > 0" class="search-results-container">
    <n-infinite-scroll
      class="search-results"
      @load="handleLoadMore"
      :scrollbar-props="{ trigger: 'none' }"
    >
      <SearchResultItem
        v-for="(item, index) in items"
        :key="`${item.subtitle}${item.title}`"
        :item="item"
        :is-selected="selectedIndex === index"
        @click="handleItemClick"
        @action-click="handleActionClick"
        @mouse-enter="selectedIndex = index"
      />
    </n-infinite-scroll>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { NInfiniteScroll } from 'naive-ui'
import SearchResultItem, { type SearchItem } from './SearchResultItem.vue'

// Types
interface Action {
  type: string
  title: string
}

// Props
interface Props {
  items: SearchItem[]
  modelValue?: number
}

const props = withDefaults(defineProps<Props>(), {
  modelValue: 0
})

// Emits
const emit = defineEmits<{
  'update:modelValue': [value: number]
  'item-click': [item: SearchItem]
  'action-click': [action: Action, item: SearchItem]
  'load-more': []
}>()

// State
const selectedIndex = ref(props.modelValue)

// Watch for external changes
watch(() => props.modelValue, (newVal) => {
  selectedIndex.value = newVal
})

// Watch for internal changes
watch(selectedIndex, (newVal) => {
  emit('update:modelValue', newVal)
})

// Methods
const handleItemClick = (item: SearchItem) => {
  emit('item-click', item)
}

const handleActionClick = (action: Action, item: SearchItem) => {
  emit('action-click', action, item)
}

const handleLoadMore = () => {
  emit('load-more')
}

const scrollToSelected = () => {
  nextTick(() => {
    const container = document.querySelector('.search-results')
    const selectedElement = container?.querySelector('.search-item.selected')
    if (selectedElement) {
      selectedElement.scrollIntoView({
        behavior: 'smooth',
        block: 'nearest'
      })
    }
  })
}

// Expose methods
defineExpose({
  scrollToSelected,
  selectedIndex
})
</script>

<style scoped>
.search-results-container {
  flex: 1;
  overflow: hidden;
  position: relative;
  background-color: var(--theme-background);
}

.search-results {
  height: 100%;
  overflow-y: auto;
  scrollbar-width: thin;
  padding-right: 2px;
}
</style>
