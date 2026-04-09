import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

export interface FrontendSettings {
  api_base_url: string;
  has_completed_setup: boolean;
  shortcut: string;
  transcription_model: string;
  post_processing_model: string;
  preserve_clipboard: boolean;
}

export const settings = writable<FrontendSettings | null>(null);

export async function loadSettings(): Promise<FrontendSettings> {
  const s = await invoke<FrontendSettings>('get_settings');
  settings.set(s);
  return s;
}

export async function validateApiKey(key: string, baseUrl: string): Promise<boolean> {
  return invoke<boolean>('validate_api_key', { key, baseUrl });
}

export async function saveSettings(key: string, baseUrl: string): Promise<void> {
  await invoke('save_settings', { key, baseUrl });
  settings.update((s) => (s ? { ...s, has_completed_setup: true } : s));
}

export async function updateSettings(opts: {
  shortcut: string;
  transcriptionModel: string;
  postProcessingModel: string;
  preserveClipboard: boolean;
}): Promise<void> {
  await invoke('update_settings', {
    shortcut: opts.shortcut,
    transcription_model: opts.transcriptionModel,
    post_processing_model: opts.postProcessingModel,
    preserve_clipboard: opts.preserveClipboard,
  });
  settings.update((s) => s ? {
    ...s,
    shortcut: opts.shortcut,
    transcription_model: opts.transcriptionModel,
    post_processing_model: opts.postProcessingModel,
    preserve_clipboard: opts.preserveClipboard,
  } : s);
}
