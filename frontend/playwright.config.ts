import {defineConfig} from '@playwright/test';
const port=Number(process.env.PW_TEST_PORT??4173);
if(!Number.isInteger(port)||port<1024||port>65535)throw new Error('PW_TEST_PORT must be an integer between 1024 and 65535');
const url=`http://127.0.0.1:${port}`;
export default defineConfig({testDir:'./tests/browser',fullyParallel:false,workers:1,timeout:60000,expect:{timeout:15000},use:{baseURL:url,browserName:'chromium',viewport:{width:1440,height:1000},trace:'retain-on-failure'},webServer:{command:`npm run preview -- --port ${port}`,url,reuseExistingServer:false}});
