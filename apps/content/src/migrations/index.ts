import * as migration_20260819_073853_initial_app_data from './20260819_073853_initial_app_data';

export const migrations = [
  {
    up: migration_20260819_073853_initial_app_data.up,
    down: migration_20260819_073853_initial_app_data.down,
    name: '20260819_073853_initial_app_data'
  },
];
