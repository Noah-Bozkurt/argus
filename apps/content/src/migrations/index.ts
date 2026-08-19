import * as migration_20260819_073853_initial_app_data from './20260819_073853_initial_app_data';
import * as migration_20260819_084821_cms_v1 from './20260819_084821_cms_v1';

export const migrations = [
  {
    up: migration_20260819_073853_initial_app_data.up,
    down: migration_20260819_073853_initial_app_data.down,
    name: '20260819_073853_initial_app_data',
  },
  {
    up: migration_20260819_084821_cms_v1.up,
    down: migration_20260819_084821_cms_v1.down,
    name: '20260819_084821_cms_v1'
  },
];
