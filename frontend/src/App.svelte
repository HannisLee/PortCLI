<script lang="ts">
  import './app.css'
  import { onMount } from 'svelte'
  import { api, type ForwardRule, type WebUIConfig } from './lib/api'
  import RuleForm from './components/RuleForm.svelte'
  import RuleItem from './components/RuleItem.svelte'
  import LogViewer from './components/LogViewer.svelte'
  import LoginForm from './components/LoginForm.svelte'

  let rules: ForwardRule[] = []
  let status: Record<string, boolean> = {}
  let showForm = false
  let logRuleId = ''
  let loading = true
  let needsAuth = false
  let webuiConfig: WebUIConfig | null = null

  async function refresh() {
    try {
      const [r, s] = await Promise.all([api.getRules(), api.getStatus()])
      rules = r || []
      status = s || {}
      needsAuth = false
    } catch (e: any) {
      if (e?.message === 'UNAUTHORIZED') {
        needsAuth = true
      } else {
        console.error('refresh failed:', e)
      }
    } finally {
      loading = false
    }
  }

  async function loadWebUIConfig() {
    try {
      webuiConfig = await api.getWebUIConfig()
    } catch {
      // ignore
    }
  }

  function handleFormSubmit() {
    showForm = false
    refresh()
  }

  function handleViewLogs(id: string) {
    logRuleId = id
  }

  function openWebUI() {
    if (webuiConfig) {
      window.open(`http://localhost:${webuiConfig.port}`, '_blank')
    }
  }

  onMount(() => {
    refresh()
    loadWebUIConfig()
  })
</script>

{#if needsAuth && !api.isWailsMode}
  <LoginForm />
{:else}
  <main class="h-full bg-gray-50 flex flex-col">
    <header class="bg-white border-b px-6 py-4 flex items-center justify-between flex-shrink-0">
      <h1 class="text-lg font-semibold text-gray-800">PortHannis</h1>
      <div class="flex items-center gap-3">
        {#if webuiConfig && webuiConfig.enabled}
          {#if api.isWailsMode}
            <button
              on:click={openWebUI}
              class="px-3 py-1.5 text-xs text-blue-600 border border-blue-200 rounded hover:bg-blue-50 flex items-center gap-1"
              title="在浏览器中打开 WebUI"
            >
              <span class="w-2 h-2 bg-green-500 rounded-full inline-block"></span>
              WebUI :{webuiConfig.port}
            </button>
          {:else}
            <span class="text-xs text-gray-400">WebUI :{webuiConfig.port}</span>
          {/if}
        {/if}
        <button
          on:click={() => (showForm = true)}
          class="px-4 py-2 text-sm bg-blue-600 text-white rounded hover:bg-blue-700"
        >
          + 添加规则
        </button>
      </div>
    </header>

    <div class="flex-1 overflow-y-auto p-6">
      {#if loading}
        <div class="text-center text-gray-400 py-12">加载中...</div>
      {:else if rules.length === 0}
        <div class="text-center text-gray-400 py-12">
          <p class="text-lg mb-2">暂无转发规则</p>
          <p class="text-sm">点击右上角 "添加规则" 开始使用</p>
        </div>
      {:else}
        <div class="space-y-3 max-w-2xl mx-auto">
          {#each rules as rule (rule.id)}
            <RuleItem
              {rule}
              running={status[rule.id] || false}
              on:refresh={refresh}
              on:viewLogs={(e) => handleViewLogs(e.detail)}
            />
          {/each}
        </div>
      {/if}
    </div>
  </main>

  {#if showForm}
    <RuleForm on:submit={handleFormSubmit} on:cancel={() => (showForm = false)} />
  {/if}

  {#if logRuleId}
    <LogViewer ruleId={logRuleId} on:close={() => { logRuleId = ''; refresh(); }} />
  {/if}
{/if}
