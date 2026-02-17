<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as monaco from 'monaco-editor';
  import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';

  interface Props {
    value?: string;
    language?: string;
    theme?: 'light' | 'dark' | 'auto';
    placeholder?: string;
    readonly?: boolean;
    minHeight?: number;
    onchange?: (value: string) => void;
    onrun?: () => void;
  }

  let {
    value = '',
    language = 'php',
    theme = 'auto',
    placeholder = '',
    readonly = false,
    minHeight = 150,
    onchange,
    onrun,
  }: Props = $props();

  let container: HTMLDivElement;
  let editor: monaco.editor.IStandaloneCodeEditor | null = null;
  let isUpdatingFromProp = false;

  // Set up Monaco environment
  self.MonacoEnvironment = {
    getWorker: function (_moduleId: string, _label: string) {
      return new editorWorker();
    }
  };

  // Register php-plain language for PHP without opening tag requirement
  monaco.languages.register({ id: 'php-plain' });

  monaco.languages.setMonarchTokensProvider('php-plain', {
    defaultToken: '',
    tokenPostfix: '',

    keywords: [
      'abstract', 'and', 'array', 'as', 'break', 'callable', 'case', 'catch',
      'class', 'clone', 'const', 'continue', 'declare', 'default', 'die', 'do',
      'echo', 'else', 'elseif', 'empty', 'enddeclare', 'endfor', 'endforeach',
      'endif', 'endswitch', 'endwhile', 'eval', 'exit', 'extends', 'final',
      'finally', 'fn', 'for', 'foreach', 'function', 'global', 'goto', 'if',
      'implements', 'include', 'include_once', 'instanceof', 'insteadof',
      'interface', 'isset', 'list', 'match', 'namespace', 'new', 'or', 'print',
      'private', 'protected', 'public', 'readonly', 'require', 'require_once',
      'return', 'static', 'switch', 'throw', 'trait', 'try', 'unset', 'use',
      'var', 'while', 'xor', 'yield', 'from',
      '__CLASS__', '__DIR__', '__FILE__', '__FUNCTION__', '__LINE__',
      '__METHOD__', '__NAMESPACE__', '__TRAIT__',
      'true', 'false', 'null', 'TRUE', 'FALSE', 'NULL',
    ],

    tokenizer: {
      root: [
        [/[a-zA-Z_]\w*/, { cases: { '@keywords': 'keyword', '@default': 'identifier' }}],
        [/\$[a-zA-Z_]\w*/, 'variable'],
        [/"([^"\\]|\\.)*$/, 'string.invalid'],
        [/'([^'\\]|\\.)*$/, 'string.invalid'],
        [/"/, 'string', '@string_double'],
        [/'/, 'string', '@string_single'],
        [/<<<\s*["']?(\w+)["']?/, 'string', '@heredoc.$1'],
        [/[0-9]+(\.[0-9]+)?/, 'number'],
        [/\/\/.*$/, 'comment'],
        [/#.*$/, 'comment'],
        [/\/\*/, 'comment', '@comment'],
        [/->|::/, 'delimiter'],
        [/[{}()\[\]]/, '@brackets'],
        [/[;,.]/, 'delimiter'],
        [/[+\-*\/%&|^!~=<>?:]/, 'operator'],
      ],
      string_double: [
        [/\$[a-zA-Z_]\w*/, 'variable'],
        [/[^"$\\]+/, 'string'],
        [/\\./, 'string.escape'],
        [/"/, 'string', '@pop'],
      ],
      string_single: [
        [/[^'\\]+/, 'string'],
        [/\\./, 'string.escape'],
        [/'/, 'string', '@pop'],
      ],
      heredoc: [
        [/^(\w+);?$/, { cases: { '$1==$S2': { token: 'string', next: '@pop' }, '@default': 'string' }}],
        [/./, 'string'],
      ],
      comment: [
        [/[^/*]+/, 'comment'],
        [/\*\//, 'comment', '@pop'],
        [/[/*]/, 'comment'],
      ],
    },
  });

  // Define custom themes
  function defineThemes() {
    monaco.editor.defineTheme('burd-light', {
      base: 'vs',
      inherit: true,
      rules: [
        { token: 'comment', foreground: '6a737d', fontStyle: 'italic' },
        { token: 'keyword', foreground: 'd73a49' },
        { token: 'string', foreground: '032f62' },
        { token: 'number', foreground: '005cc5' },
        { token: 'variable', foreground: 'e36209' },
        { token: 'type', foreground: '6f42c1' },
      ],
      colors: {
        'editor.background': '#ffffff',
        'editor.foreground': '#24292e',
        'editor.lineHighlightBackground': '#f6f8fa',
        'editorLineNumber.foreground': '#959da5',
        'editorLineNumber.activeForeground': '#24292e',
        'editor.selectionBackground': '#c8e1ff',
        'editor.inactiveSelectionBackground': '#e8eaed',
      }
    });

    monaco.editor.defineTheme('burd-dark', {
      base: 'vs-dark',
      inherit: true,
      rules: [
        { token: 'comment', foreground: '6a737d', fontStyle: 'italic' },
        { token: 'keyword', foreground: 'ff7b72' },
        { token: 'string', foreground: 'a5d6ff' },
        { token: 'number', foreground: '79c0ff' },
        { token: 'variable', foreground: 'ffa657' },
        { token: 'type', foreground: 'd2a8ff' },
      ],
      colors: {
        'editor.background': '#1c1c1e',
        'editor.foreground': '#c9d1d9',
        'editor.lineHighlightBackground': '#2c2c2e',
        'editorLineNumber.foreground': '#6e7681',
        'editorLineNumber.activeForeground': '#c9d1d9',
        'editor.selectionBackground': '#264f78',
        'editor.inactiveSelectionBackground': '#3a3a3c',
      }
    });
  }

  function getEffectiveTheme(): 'burd-light' | 'burd-dark' {
    if (theme === 'auto') {
      // Check data-theme attribute first, then system preference
      const dataTheme = document.documentElement.getAttribute('data-theme');
      if (dataTheme === 'dark') return 'burd-dark';
      if (dataTheme === 'light') return 'burd-light';
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'burd-dark' : 'burd-light';
    }
    return theme === 'dark' ? 'burd-dark' : 'burd-light';
  }

  function updateTheme() {
    if (editor) {
      monaco.editor.setTheme(getEffectiveTheme());
    }
  }

  onMount(() => {
    defineThemes();

    editor = monaco.editor.create(container, {
      value: value || '',
      language,
      theme: getEffectiveTheme(),
      minimap: { enabled: false },
      fontSize: 13,
      fontFamily: "'SF Mono', 'Menlo', 'Monaco', 'Courier New', monospace",
      lineNumbers: 'on',
      renderLineHighlight: 'line',
      scrollBeyondLastLine: false,
      automaticLayout: true,
      tabSize: 4,
      insertSpaces: true,
      wordWrap: 'on',
      padding: { top: 12, bottom: 12 },
      readOnly: readonly,
      scrollbar: {
        vertical: 'auto',
        horizontal: 'auto',
        verticalScrollbarSize: 10,
        horizontalScrollbarSize: 10,
      },
      overviewRulerBorder: false,
      hideCursorInOverviewRuler: true,
      renderWhitespace: 'none',
      contextmenu: true,
      quickSuggestions: false,
      suggestOnTriggerCharacters: false,
      acceptSuggestionOnEnter: 'off',
      tabCompletion: 'off',
      wordBasedSuggestions: 'off',
      parameterHints: { enabled: false },
    });

    // Handle content changes
    editor.onDidChangeModelContent(() => {
      if (!isUpdatingFromProp) {
        const newValue = editor?.getValue() || '';
        onchange?.(newValue);
      }
    });

    // Add Cmd/Ctrl+Enter keybinding
    editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, () => {
      onrun?.();
    });

    // Watch for theme changes
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        if (mutation.attributeName === 'data-theme') {
          updateTheme();
        }
      }
    });
    observer.observe(document.documentElement, { attributes: true });

    // Watch for system theme changes
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleMediaChange = () => {
      if (theme === 'auto') {
        updateTheme();
      }
    };
    mediaQuery.addEventListener('change', handleMediaChange);

    return () => {
      observer.disconnect();
      mediaQuery.removeEventListener('change', handleMediaChange);
    };
  });

  onDestroy(() => {
    editor?.dispose();
  });

  // Update editor value when prop changes
  $effect(() => {
    if (editor && value !== editor.getValue()) {
      isUpdatingFromProp = true;
      editor.setValue(value || '');
      isUpdatingFromProp = false;
    }
  });

  // Update readonly state
  $effect(() => {
    if (editor) {
      editor.updateOptions({ readOnly: readonly });
    }
  });

  // Focus the editor
  export function focus() {
    editor?.focus();
  }

  // Get the editor instance
  export function getEditor() {
    return editor;
  }
</script>

<div
  class="monaco-container"
  bind:this={container}
  style="min-height: {minHeight}px"
></div>

<style>
  .monaco-container {
    width: 100%;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid #e0e0e0;
  }

  :global(:root[data-theme="dark"]) .monaco-container {
    border-color: #38383a;
  }
</style>
