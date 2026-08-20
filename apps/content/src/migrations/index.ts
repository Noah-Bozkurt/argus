import * as migration_20260819_073853_initial_app_data from './20260819_073853_initial_app_data';
import * as migration_20260819_084821_cms_v1 from './20260819_084821_cms_v1';
import * as migration_20260820_045713_page_component_schemas_v1 from './20260820_045713_page_component_schemas_v1';
import * as migration_20260820_053757_media_library_v1 from './20260820_053757_media_library_v1';
import * as migration_20260820_061644_forms_submissions_v1 from './20260820_061644_forms_submissions_v1';

export const migrations = [
  {
    up: migration_20260819_073853_initial_app_data.up,
    down: migration_20260819_073853_initial_app_data.down,
    name: '20260819_073853_initial_app_data',
  },
  {
    up: migration_20260819_084821_cms_v1.up,
    down: migration_20260819_084821_cms_v1.down,
    name: '20260819_084821_cms_v1',
  },
  {
    up: migration_20260820_045713_page_component_schemas_v1.up,
    down: migration_20260820_045713_page_component_schemas_v1.down,
    name: '20260820_045713_page_component_schemas_v1',
  },
  {
    up: migration_20260820_053757_media_library_v1.up,
    down: migration_20260820_053757_media_library_v1.down,
    name: '20260820_053757_media_library_v1',
  },
  {
    up: migration_20260820_061644_forms_submissions_v1.up,
    down: migration_20260820_061644_forms_submissions_v1.down,
    name: '20260820_061644_forms_submissions_v1'
  },
];
