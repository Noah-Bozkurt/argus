import * as migration_20260819_073853_initial_app_data from './20260819_073853_initial_app_data';
import * as migration_20260819_084821_cms_v1 from './20260819_084821_cms_v1';
import * as migration_20260820_045713_page_component_schemas_v1 from './20260820_045713_page_component_schemas_v1';
import * as migration_20260820_053757_media_library_v1 from './20260820_053757_media_library_v1';
import * as migration_20260820_061644_forms_submissions_v1 from './20260820_061644_forms_submissions_v1';
import * as migration_20260820_073522_cms_media_fields_v1 from './20260820_073522_cms_media_fields_v1';
import * as migration_20260820_184500_shared_auth_roles from './20260820_184500_shared_auth_roles';
import * as migration_20260919_121733_site_publishing from './20260919_121733_site_publishing';

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
    name: '20260820_061644_forms_submissions_v1',
  },
  {
    up: migration_20260820_073522_cms_media_fields_v1.up,
    down: migration_20260820_073522_cms_media_fields_v1.down,
    name: '20260820_073522_cms_media_fields_v1',
  },
  {
    up: migration_20260820_184500_shared_auth_roles.up,
    down: migration_20260820_184500_shared_auth_roles.down,
    name: '20260820_184500_shared_auth_roles',
  },
  {
    up: migration_20260919_121733_site_publishing.up,
    down: migration_20260919_121733_site_publishing.down,
    name: '20260919_121733_site_publishing'
  },
];
