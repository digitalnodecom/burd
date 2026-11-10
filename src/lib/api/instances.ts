import { invoke } from '@tauri-apps/api/core';

export interface InstanceSettings {
  id: string;
  name: string;
  port: number;
  version: string;
  config: Record<string, string>;
  autoStart: boolean;
}
export const saveInstanceSettings = (settings: InstanceSettings) => invoke<void>('save_instance_settings', { ...settings });
export const deleteStack = (id: string, deleteInstances: boolean) => invoke<string[]>('delete_stack', { id, deleteInstances });
