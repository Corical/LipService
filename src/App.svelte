<script lang="ts">
  import { onMount } from 'svelte';
  import { loadSettings, validateApiKey, saveSettings } from './lib/stores/settings';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let apiKey = $state('');
  let baseUrl = $state('');
  let showBaseUrl = $state(false);
  let error = $state('');
  let isValidating = $state(false);
  let isComplete = $state(false);

  onMount(async () => {
    try {
      const s = await loadSettings();
      if (s.has_completed_setup) {
        isComplete = true;
        await getCurrentWindow().hide();
      }
      if (s.api_base_url && s.api_base_url !== 'https://api.groq.com/openai/v1') {
        baseUrl = s.api_base_url;
        showBaseUrl = true;
      }
    } catch (e) {
      // First run — no settings yet
    }
  });

  async function handleSubmit() {
    error = '';
    isValidating = true;

    try {
      const url = baseUrl.trim() || 'https://api.groq.com/openai/v1';
      const valid = await validateApiKey(apiKey.trim(), url);

      if (!valid) {
        error = 'Invalid API key. Please check and try again.';
        isValidating = false;
        return;
      }

      await saveSettings(apiKey.trim(), url);
      isComplete = true;
      await getCurrentWindow().hide();
    } catch (e) {
      error = `Error: ${e}`;
    } finally {
      isValidating = false;
    }
  }
</script>

{#if !isComplete}
  <div class="container">
    <div class="card">
      <h1>VTT Setup</h1>
      <p class="subtitle">Voice-to-Text for Windows</p>

      <div class="form-group">
        <label for="api-key">Groq API Key</label>
        <input
          id="api-key"
          type="password"
          bind:value={apiKey}
          placeholder="gsk_..."
          disabled={isValidating}
        />
        <p class="hint">
          Get a free key at <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a>
        </p>
      </div>

      <button
        class="toggle-advanced"
        onclick={() => (showBaseUrl = !showBaseUrl)}
        type="button"
      >
        {showBaseUrl ? 'Hide' : 'Show'} advanced options
      </button>

      {#if showBaseUrl}
        <div class="form-group">
          <label for="base-url">API Base URL</label>
          <input
            id="base-url"
            type="text"
            bind:value={baseUrl}
            placeholder="https://api.groq.com/openai/v1"
            disabled={isValidating}
          />
        </div>
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <button
        class="primary"
        onclick={handleSubmit}
        disabled={!apiKey.trim() || isValidating}
      >
        {isValidating ? 'Validating...' : 'Save & Start'}
      </button>

      <p class="footer-hint">
        After setup, hold <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Space</kbd> to dictate.
      </p>
    </div>
  </div>
{/if}

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    background: #0f0f0f;
  }

  .container {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    background: #0f0f0f;
    color: #e0e0e0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  }

  .card {
    background: #1a1a1a;
    border-radius: 12px;
    padding: 2rem;
    width: 380px;
    border: 1px solid #2a2a2a;
  }

  h1 {
    margin: 0 0 0.25rem;
    font-size: 1.5rem;
    color: #fff;
  }

  .subtitle {
    margin: 0 0 1.5rem;
    color: #888;
    font-size: 0.875rem;
  }

  .form-group {
    margin-bottom: 1rem;
  }

  label {
    display: block;
    margin-bottom: 0.375rem;
    font-size: 0.875rem;
    color: #aaa;
  }

  input {
    width: 100%;
    padding: 0.625rem;
    background: #111;
    border: 1px solid #333;
    border-radius: 6px;
    color: #e0e0e0;
    font-size: 0.875rem;
    box-sizing: border-box;
  }

  input:focus {
    outline: none;
    border-color: #4a9eff;
  }

  .hint {
    margin: 0.375rem 0 0;
    font-size: 0.75rem;
    color: #666;
  }

  .hint a {
    color: #4a9eff;
    text-decoration: none;
  }

  .toggle-advanced {
    background: none;
    border: none;
    color: #666;
    font-size: 0.75rem;
    cursor: pointer;
    padding: 0;
    margin-bottom: 0.75rem;
  }

  .toggle-advanced:hover {
    color: #999;
  }

  .error {
    color: #ff4a4a;
    font-size: 0.8125rem;
    margin: 0.5rem 0;
  }

  .primary {
    width: 100%;
    padding: 0.75rem;
    background: #4a9eff;
    color: #fff;
    border: none;
    border-radius: 8px;
    font-size: 0.9375rem;
    font-weight: 600;
    cursor: pointer;
    margin-top: 0.5rem;
  }

  .primary:hover:not(:disabled) {
    background: #3a8eef;
  }

  .primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .footer-hint {
    text-align: center;
    margin: 1.25rem 0 0;
    font-size: 0.75rem;
    color: #555;
  }

  kbd {
    background: #252525;
    border: 1px solid #444;
    border-radius: 3px;
    padding: 0.125rem 0.375rem;
    font-family: monospace;
    font-size: 0.6875rem;
  }
</style>
