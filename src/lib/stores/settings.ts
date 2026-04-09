import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

export interface FrontendSettings {
  api_base_url: string;
  has_completed_setup: boolean;
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
