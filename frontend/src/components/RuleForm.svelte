<script lang="ts">
  import { createEventDispatcher } from 'svelte'
  import { api } from '../lib/api'

  const dispatch = createEventDispatcher<{ submit: void; cancel: void }>()

  let sourceHost = '0.0.0.0'
  let localPort = ''
  let targetHost = ''
  let targetPort = ''
  let error = ''
  let submitting = false

  function validate(): string | null {
    if (!sourceHost.trim()) {
      return '来源地址不能为空'
    }
    const lp = parseInt(localPort)
    if (isNaN(lp) || lp < 1024 || lp > 65535) {
      return '本地端口范围：1024-65535'
    }
    if (!targetHost.trim()) {
      return '目标地址不能为空'
    }
    const tp = parseInt(targetPort)
    if (isNaN(tp) || tp < 1 || tp > 65535) {
      return '目标端口范围：1-65535'
    }
    return null
  }

  async function handleSubmit() {
    error = ''
    const err = validate()
    if (err) {
      error = err
      return
    }
    submitting = true
    try {
      await api.addRule(sourceHost.trim(), parseInt(localPort), targetHost.trim(), parseInt(targetPort))
      dispatch('submit')
    } catch (e: any) {
      error = e?.message || String(e)
    } finally {
      submitting = false
    }
  }

  function handleCancel() {
    dispatch('cancel')
  }
</script>

<div class="fixed inset-0 bg-black/40 flex items-center justify-center z-50" on:click={handleCancel}>
  <div class="bg-white rounded-lg shadow-xl p-6 w-96" on:click|stopPropagation>
    <h2 class="text-lg font-semibold mb-4">添加转发规则</h2>

    {#if error}
      <div class="bg-red-50 text-red-600 text-sm px-3 py-2 rounded mb-3">{error}</div>
    {/if}

    <div class="space-y-3">
      <div>
        <label class="block text-sm text-gray-600 mb-1">来源地址</label>
        <input
          type="text"
          bind:value={sourceHost}
          placeholder="0.0.0.0（全部接口）"
          class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
        />
        <p class="text-xs text-gray-400 mt-1">本地监听地址，0.0.0.0 表示接受所有来源</p>
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">本地端口</label>
        <input
          type="number"
          bind:value={localPort}
          placeholder="1024-65535"
          class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
        />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">目标地址</label>
        <input
          type="text"
          bind:value={targetHost}
          placeholder="192.168.1.100"
          class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
        />
      </div>
      <div>
        <label class="block text-sm text-gray-600 mb-1">目标端口</label>
        <input
          type="number"
          bind:value={targetPort}
          placeholder="1-65535"
          class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
        />
      </div>
    </div>

    <div class="flex justify-end gap-2 mt-5">
      <button
        on:click={handleCancel}
        class="px-4 py-2 text-sm text-gray-600 hover:bg-gray-100 rounded"
      >
        取消
      </button>
      <button
        on:click={handleSubmit}
        disabled={submitting}
        class="px-4 py-2 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
      >
        {submitting ? '添加中...' : '添加'}
      </button>
    </div>
  </div>
</div>
