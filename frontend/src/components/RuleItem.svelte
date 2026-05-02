<script lang="ts">
  import type { ForwardRule } from '../lib/api'
  import { api } from '../lib/api'
  import { createEventDispatcher } from 'svelte'

  export let rule: ForwardRule
  export let running = false

  const dispatch = createEventDispatcher<{ refresh: void; viewLogs: string }>

  let deleting = false

  async function handleToggle() {
    try {
      await api.toggleRule(rule.id, !rule.enabled)
      dispatch('refresh')
    } catch (e: any) {
      console.error('toggle failed:', e)
    }
  }

  async function handleDelete() {
    if (deleting) {
      try {
        await api.deleteRule(rule.id)
        dispatch('refresh')
      } catch (e: any) {
        console.error('delete failed:', e)
      }
      deleting = false
    } else {
      deleting = true
      setTimeout(() => (deleting = false), 3000)
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return bytes + ' B'
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
    return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  }
</script>

<div class="bg-white rounded-lg border p-4 flex items-center justify-between gap-4">
  <div class="flex items-center gap-3 min-w-0">
    <span class="w-3 h-3 rounded-full flex-shrink-0 {rule.enabled && running ? 'bg-green-500' : 'bg-gray-300'}"></span>
    <div class="min-w-0">
      <div class="font-mono text-sm">
        {rule.sourceHost || '0.0.0.0'}:{rule.localPort} <span class="text-gray-400">&rarr;</span> {rule.targetHost}:{rule.targetPort}
      </div>
      <div class="text-xs text-gray-400">
        {rule.enabled && running ? '运行中' : '已停止'}
      </div>
    </div>
  </div>

  <div class="flex items-center gap-2 flex-shrink-0">
    <button
      on:click={() => dispatch('viewLogs', rule.id)}
      class="px-3 py-1.5 text-xs text-gray-600 hover:bg-gray-100 rounded"
    >
      日志
    </button>
    <button
      on:click={handleToggle}
      class="px-3 py-1.5 text-xs rounded {rule.enabled ? 'text-yellow-600 hover:bg-yellow-50' : 'text-green-600 hover:bg-green-50'}"
    >
      {rule.enabled ? '禁用' : '启用'}
    </button>
    <button
      on:click={handleDelete}
      class="px-3 py-1.5 text-xs rounded {deleting ? 'bg-red-600 text-white' : 'text-red-600 hover:bg-red-50'}"
    >
      {deleting ? '确认删除?' : '删除'}
    </button>
  </div>
</div>
