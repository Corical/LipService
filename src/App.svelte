<script lang="ts">
  import { onMount } from 'svelte';
  import { loadSettings, validateApiKey, saveSettings, updateSettings } from './lib/stores/settings';
  import type { FrontendSettings } from './lib/stores/settings';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let apiKey = $state('');
  let baseUrl = $state('');
  let showBaseUrl = $state(false);
  let error = $state('');
  let isValidating = $state(false);
  let settingsSaved = $state(false);

  // Settings state
  let tab = $state<'setup' | 'settings'>('setup');
  let shortcut = $state('CmdOrCtrl+Shift+Space');
  let pendingShortcut = $state(''); // preview only, not applied until save
  let transcriptionModel = $state('whisper-large-v3');
  let postProcessingModel = $state('llama-3.3-70b-versatile');
  let preserveClipboard = $state(true);
  let capturingShortcut = $state(false);
  let capturedKeys = $state('');

  const TRANSCRIPTION_MODELS = [
    'whisper-large-v3',
    'whisper-large-v3-turbo',
  ];

  const POST_PROCESSING_MODELS = [
    'llama-3.3-70b-versatile',
    'llama-3.1-8b-instant',
    'llama-3.1-70b-versatile',
    'mixtral-8x7b-32768',
    'gemma2-9b-it',
  ];

  function resetUIState() {
    capturingShortcut = false;
    capturedKeys = '';
    pendingShortcut = '';
    error = '';
    settingsSaved = false;
  }

  async function loadFromDisk() {
    try {
      const s = await loadSettings();
      if (s.has_completed_setup) {
        tab = 'settings';
        shortcut = s.shortcut || 'CmdOrCtrl+Shift+Space';
        transcriptionModel = s.transcription_model || 'whisper-large-v3';
        postProcessingModel = s.post_processing_model || 'llama-3.3-70b-versatile';
        preserveClipboard = s.preserve_clipboard ?? true;
        if (s.api_base_url && s.api_base_url !== 'https://api.groq.com/openai/v1') {
          baseUrl = s.api_base_url;
          showBaseUrl = true;
        }
      }
    } catch (e) {
      // First run
    }
  }

  onMount(async () => {
    await loadFromDisk();

    // Reset UI state when window is shown (e.g., from tray)
    const win = getCurrentWindow();
    await win.onFocusChanged(({ payload: focused }) => {
      if (focused) {
        resetUIState();
        loadFromDisk(); // reload saved values
      }
    });
  });

  async function handleSetupSubmit() {
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
      tab = 'settings';
      await getCurrentWindow().hide();
    } catch (e) {
      error = `Error: ${e}`;
    } finally {
      isValidating = false;
    }
  }

  async function handleSettingsSave() {
    const shortcutToSave = pendingShortcut || shortcut;
    try {
      await updateSettings({
        shortcut: shortcutToSave,
        transcriptionModel,
        postProcessingModel,
        preserveClipboard,
      });
      shortcut = shortcutToSave; // apply to display
      pendingShortcut = '';
      settingsSaved = true;
      setTimeout(() => { settingsSaved = false; }, 2000);
    } catch (e) {
      error = `Error: ${e}`;
    }
  }

  async function startShortcutCapture() {
    try {
      const { unregisterAll } = await import('@tauri-apps/plugin-global-shortcut');
      await unregisterAll();
    } catch (e) {
      console.error('[CAPTURE] failed to unregister:', e);
    }
    pendingShortcut = '';
    capturingShortcut = true;
    capturedKeys = '';
  }

  async function cancelShortcutCapture() {
    capturingShortcut = false;
    pendingShortcut = '';
    capturedKeys = '';
    // Re-register the current shortcut since we unregistered it
    await handleSettingsSave();
  }

  function handleShortcutKeydown(e: KeyboardEvent) {
    if (!capturingShortcut) return;
    e.preventDefault();
    e.stopPropagation();

    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push('CmdOrCtrl');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');

    const key = e.key;
    if (!['Control', 'Shift', 'Alt', 'Meta'].includes(key)) {
      const keyMap: Record<string, string> = {
        ' ': 'Space', 'ArrowUp': 'Up', 'ArrowDown': 'Down',
        'ArrowLeft': 'Left', 'ArrowRight': 'Right', 'Escape': 'Escape',
        'Enter': 'Enter', 'Backspace': 'Backspace', 'Delete': 'Delete',
        'Tab': 'Tab', 'Home': 'Home', 'End': 'End',
        'PageUp': 'PageUp', 'PageDown': 'PageDown',
      };
      const mappedKey = keyMap[key] || (key.length === 1 ? key.toUpperCase() : key);
      parts.push(mappedKey);

      pendingShortcut = parts.join('+');
      capturingShortcut = false;
    } else {
      capturedKeys = parts.join('+') + '+...';
    }
  }

  function displayShortcut(s: string): string {
    return s.replace('CmdOrCtrl', 'Ctrl').replace(/\+/g, ' + ');
  }
</script>

<svelte:window onkeydown={handleShortcutKeydown} />

{#if tab === 'setup'}
  <div class="container">
    <div class="card">
      <h1>LipService</h1>
      <p class="subtitle">Voice-to-Text for Windows</p>

      <div class="form-group">
        <label for="api-key">Groq API Key</label>
        <input id="api-key" type="password" bind:value={apiKey}
          placeholder="gsk_..." disabled={isValidating} />
        <p class="hint">
          Get a free key at <a href="https://console.groq.com" target="_blank" rel="noopener">console.groq.com</a>
        </p>
      </div>

      <button class="toggle-advanced" onclick={() => (showBaseUrl = !showBaseUrl)} type="button">
        {showBaseUrl ? 'Hide' : 'Show'} advanced options
      </button>

      {#if showBaseUrl}
        <div class="form-group">
          <label for="base-url">API Base URL</label>
          <input id="base-url" type="text" bind:value={baseUrl}
            placeholder="https://api.groq.com/openai/v1" disabled={isValidating} />
          <p class="hint">Use a custom URL for Ollama (e.g., http://localhost:11434/v1)</p>
        </div>
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <button class="primary" onclick={handleSetupSubmit}
        disabled={!apiKey.trim() || isValidating}>
        {isValidating ? 'Validating...' : 'Save & Start'}
      </button>

      <p class="footer-hint">
        After setup, tap your shortcut to start/stop dictation.
      </p>
    </div>
  </div>
{:else}
  <div class="container">
    <div class="card settings-card">
      <h1>LipService Settings</h1>

      <div class="form-group">
        <label>Shortcut</label>
        {#if capturingShortcut}
          <div class="shortcut-capture">
            <span class="capture-label">{capturedKeys || 'Press your shortcut...'}</span>
            <button class="btn-small" onclick={cancelShortcutCapture}>Cancel</button>
          </div>
        {:else}
          <div class="shortcut-display">
            <kbd class="shortcut-value">
              {displayShortcut(pendingShortcut || shortcut)}
            </kbd>
            {#if pendingShortcut}
              <span class="unsaved-badge">unsaved</span>
            {/if}
            <button class="btn-small" onclick={startShortcutCapture}>Change</button>
          </div>
        {/if}
      </div>

      <div class="form-group">
        <label for="t-model">Transcription Model</label>
        <select id="t-model" bind:value={transcriptionModel}>
          {#each TRANSCRIPTION_MODELS as m}
            <option value={m}>{m}</option>
          {/each}
        </select>
      </div>

      <div class="form-group">
        <label for="pp-model">Post-Processing Model</label>
        <select id="pp-model" bind:value={postProcessingModel}>
          {#each POST_PROCESSING_MODELS as m}
            <option value={m}>{m}</option>
          {/each}
        </select>
      </div>

      <div class="form-group checkbox-group">
        <label>
          <input type="checkbox" bind:checked={preserveClipboard} />
          Preserve clipboard (restore after paste)
        </label>
      </div>

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <button class="primary" onclick={handleSettingsSave}>
        {settingsSaved ? 'Saved!' : 'Save Settings'}
      </button>

      <button class="secondary" onclick={() => { tab = 'setup'; }}>
        Change API Key
      </button>
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

  .settings-card {
    width: 420px;
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

  input[type="text"],
  input[type="password"],
  select {
    width: 100%;
    padding: 0.625rem;
    background: #111;
    border: 1px solid #333;
    border-radius: 6px;
    color: #e0e0e0;
    font-size: 0.875rem;
    box-sizing: border-box;
  }

  select { cursor: pointer; }

  input:focus, select:focus {
    outline: none;
    border-color: #4a9eff;
  }

  .checkbox-group label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    color: #ccc;
  }

  .checkbox-group input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: #4a9eff;
  }

  .shortcut-display, .shortcut-capture {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .shortcut-value {
    background: #252525;
    border: 1px solid #444;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-family: monospace;
    font-size: 0.875rem;
    color: #fff;
  }

  .unsaved-badge {
    font-size: 0.65rem;
    color: #f5a623;
    background: rgba(245, 166, 35, 0.15);
    border-radius: 3px;
    padding: 0.15rem 0.4rem;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .capture-label {
    background: #1a1a3a;
    border: 1px solid #4a9eff;
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    font-size: 0.875rem;
    color: #4a9eff;
    animation: pulse-border 1s ease-in-out infinite;
  }

  @keyframes pulse-border {
    0%, 100% { border-color: #4a9eff; }
    50% { border-color: #2a6ecc; }
  }

  .btn-small {
    background: #333;
    border: 1px solid #555;
    border-radius: 4px;
    color: #ccc;
    padding: 0.375rem 0.75rem;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .btn-small:hover { background: #444; }

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

  .toggle-advanced:hover { color: #999; }

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

  .primary:hover:not(:disabled) { background: #3a8eef; }
  .primary:disabled { opacity: 0.5; cursor: not-allowed; }

  .secondary {
    width: 100%;
    padding: 0.625rem;
    background: transparent;
    color: #888;
    border: 1px solid #333;
    border-radius: 8px;
    font-size: 0.8125rem;
    cursor: pointer;
    margin-top: 0.5rem;
  }

  .secondary:hover { color: #ccc; border-color: #555; }

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
