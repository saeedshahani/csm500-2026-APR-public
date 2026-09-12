import tseslint from 'typescript-eslint';
export default tseslint.config(...tseslint.configs.recommended, {files:['**/*.{ts,tsx,mjs}'],rules:{'@typescript-eslint/no-unused-vars':['error',{argsIgnorePattern:'^_'}]} });
