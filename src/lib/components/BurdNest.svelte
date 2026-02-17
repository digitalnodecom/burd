<script lang="ts">
  import { onMount } from 'svelte';
  import {
    apiCategories,
    getRandomBirdPun,
    getAllEndpointsFormatted,
    getCategoryEndpoints
  } from '$lib/data/api-docs';

  interface Props {
    show: boolean;
    onClose: () => void;
  }

  let { show, onClose }: Props = $props();

  // Terminal state
  let input = $state('');
  let outputLines = $state<string[]>([]);
  let commandHistory = $state<string[]>([]);
  let historyIndex = $state(-1);
  let inputRef = $state<HTMLInputElement>();
  let outputRef = $state<HTMLDivElement>();
  let isTyping = $state(false);

  // ASCII Art Header
  const ASCII_HEADER = `
╔══════════════════════════════════════════════════════════════════════╗
║  ____  _   _ ____  ____    _   _ _____ ____ _____                     ║
║ | __ )| | | |  _ \\|  _ \\  | \\ | | ____/ ___|_   _|                    ║
║ |  _ \\| | | | |_) | | | | |  \\| |  _| \\___ \\ | |                      ║
║ | |_) | |_| |  _ <| |_| | | |\\  | |___ ___) || |                      ║
║ |____/ \\___/|_| \\_\\____/  |_| \\_|_____|____/ |_|                      ║
║                                                                      ║
║                     \\\\                                                ║
║                    (o>    Welcome to the secret API terminal         ║
║                    //\\\\    Type 'help' to get started                ║
║                    V_/_   Type 'exit' or press ESC to leave          ║
║                                                                      ║
║                    API Server: http://localhost:19840                ║
╚══════════════════════════════════════════════════════════════════════╝`;

  const WELCOME_MESSAGE = `
${ASCII_HEADER}

Type 'help' for available commands.
`;

  // Help text
  const HELP_TEXT = `
Available commands:

  help              Show this help message
  api               List all API endpoints by category
  api <category>    Show detailed endpoints for a category
                    Categories: status, instances, domains, databases, services
  chirp             Get a random bird pun
  about             Show credits and version info
  clear             Clear the terminal
  exit              Close The Burd Nest (or press ESC)

Pro tip: The API runs on http://localhost:19840 when Burd is active.
`;

  const ABOUT_TEXT = `
╔══════════════════════════════════════════════════════════════════════╗
║                                                                      ║
║                    THE BURD NEST v1.0.0                              ║
║                                                                      ║
║   A hidden terminal for the curious developer.                       ║
║   You found the secret! Konami Code: ↑↑↓↓←→←→BA                     ║
║                                                                      ║
║   Burd - Local Development Environment Manager                       ║
║   Making local dev feel like flying.                                 ║
║                                                                      ║
║                        \\\\                                             ║
║                       (o>                                            ║
║                       //\\\\                                            ║
║                       V_/_                                           ║
║                                                                      ║
╚══════════════════════════════════════════════════════════════════════╝
`;

  // Type text with animation
  async function typeText(text: string, speed: number = 5): Promise<void> {
    isTyping = true;
    const lines = text.split('\n');

    for (const line of lines) {
      let currentLine = '';
      for (const char of line) {
        currentLine += char;
        // Update the last line being typed
        outputLines = [...outputLines.slice(0, -1), currentLine];
        await new Promise(r => setTimeout(r, speed));
      }
      outputLines = [...outputLines, ''];
    }
    // Remove the extra empty line at the end
    outputLines = outputLines.slice(0, -1);
    isTyping = false;
  }

  // Add text instantly (for command echo)
  function addLine(text: string): void {
    outputLines = [...outputLines, text];
  }

  // Add multiple lines with typing effect
  async function addOutput(text: string, animate: boolean = true): Promise<void> {
    if (animate && text.length < 500) {
      // Add empty line to start typing into
      outputLines = [...outputLines, ''];
      await typeText(text, 3);
    } else {
      // For large outputs, just add directly
      const lines = text.split('\n');
      outputLines = [...outputLines, ...lines];
    }
    scrollToBottom();
  }

  // Process command
  async function processCommand(cmd: string): Promise<void> {
    const trimmed = cmd.trim().toLowerCase();
    const parts = trimmed.split(/\s+/);
    const command = parts[0];
    const args = parts.slice(1);

    // Add command to history
    if (trimmed) {
      commandHistory = [...commandHistory, cmd];
      historyIndex = commandHistory.length;
    }

    // Echo the command
    addLine(`burd> ${cmd}`);

    if (!command) {
      return;
    }

    switch (command) {
      case 'help':
        await addOutput(HELP_TEXT);
        break;

      case 'api':
        if (args.length === 0) {
          await addOutput(getAllEndpointsFormatted(), false);
        } else {
          const categoryOutput = getCategoryEndpoints(args[0]);
          if (categoryOutput) {
            await addOutput(categoryOutput, false);
          } else {
            await addOutput(`Unknown category: ${args[0]}\nAvailable: status, instances, domains, databases, services`);
          }
        }
        break;

      case 'chirp':
        const pun = getRandomBirdPun();
        await addOutput(`\n  🐦 ${pun}\n`);
        break;

      case 'about':
        await addOutput(ABOUT_TEXT, false);
        break;

      case 'clear':
        outputLines = [];
        await addOutput(ASCII_HEADER, false);
        break;

      case 'exit':
      case 'quit':
        onClose();
        break;

      default:
        await addOutput(`Unknown command: ${command}\nType 'help' for available commands.`);
    }

    scrollToBottom();
  }

  // Handle input keydown
  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter' && !isTyping) {
      e.preventDefault();
      const cmd = input;
      input = '';
      processCommand(cmd);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (historyIndex > 0) {
        historyIndex--;
        input = commandHistory[historyIndex] || '';
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (historyIndex < commandHistory.length - 1) {
        historyIndex++;
        input = commandHistory[historyIndex] || '';
      } else {
        historyIndex = commandHistory.length;
        input = '';
      }
    }
  }

  // Handle overlay keydown (for ESC)
  function handleOverlayKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  // Scroll output to bottom
  function scrollToBottom(): void {
    setTimeout(() => {
      if (outputRef) {
        outputRef.scrollTop = outputRef.scrollHeight;
      }
    }, 10);
  }

  // Focus input when shown
  $effect(() => {
    if (show && inputRef) {
      setTimeout(() => {
        inputRef?.focus();
        // Show welcome message
        if (outputLines.length === 0) {
          addOutput(WELCOME_MESSAGE, false);
        }
      }, 100);
    }
  });

  // Click to focus
  function handleTerminalClick(): void {
    inputRef?.focus();
  }
</script>

{#if show}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="nest-overlay"
    onkeydown={handleOverlayKeydown}
    role="dialog"
    aria-modal="true"
    aria-label="The Burd Nest Terminal"
    tabindex="0"
  >
    <div class="crt-container">
      <div class="terminal" onclick={handleTerminalClick} role="presentation">
        <div class="terminal-header">
          <span class="terminal-title">~ THE BURD NEST ~</span>
          <button class="close-btn" onclick={onClose} aria-label="Close terminal">[X]</button>
        </div>

        <div class="terminal-body" bind:this={outputRef}>
          {#each outputLines as line}
            <pre class="output-line">{line}</pre>
          {/each}

          <div class="input-line">
            <span class="prompt">burd&gt;</span>
            <input
              bind:this={inputRef}
              bind:value={input}
              onkeydown={handleKeydown}
              class="terminal-input"
              type="text"
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
              disabled={isTyping}
            />
            <span class="cursor" class:blink={!isTyping}>_</span>
          </div>
        </div>

        <div class="scanlines"></div>
      </div>
    </div>
  </div>
{/if}

<style>
  .nest-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.95);
    z-index: 99999;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: fadeIn 0.3s ease-out;
  }

  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .crt-container {
    position: relative;
    width: 90%;
    max-width: 900px;
    height: 85vh;
    animation: crtOn 0.5s ease-out;
  }

  @keyframes crtOn {
    0% {
      transform: scale(1, 0.01);
      filter: brightness(10);
    }
    50% {
      transform: scale(1, 1);
      filter: brightness(10);
    }
    100% {
      transform: scale(1, 1);
      filter: brightness(1);
    }
  }

  .terminal {
    position: relative;
    background: #1a1000;
    border: 3px solid #ffb000;
    border-radius: 12px;
    width: 100%;
    height: 100%;
    font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
    color: #ffb000;
    box-shadow:
      0 0 30px rgba(255, 176, 0, 0.4),
      0 0 60px rgba(255, 176, 0, 0.2),
      inset 0 0 100px rgba(255, 176, 0, 0.05);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* CRT flicker effect */
  .terminal::before {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(
      transparent 50%,
      rgba(0, 0, 0, 0.05) 50%
    );
    background-size: 100% 4px;
    pointer-events: none;
    z-index: 10;
    border-radius: 10px;
  }

  /* Scanlines overlay */
  .scanlines {
    position: absolute;
    inset: 0;
    background: repeating-linear-gradient(
      0deg,
      rgba(0, 0, 0, 0.15),
      rgba(0, 0, 0, 0.15) 1px,
      transparent 1px,
      transparent 2px
    );
    pointer-events: none;
    z-index: 11;
    border-radius: 10px;
    animation: flicker 0.15s infinite;
  }

  @keyframes flicker {
    0% { opacity: 0.97; }
    50% { opacity: 1; }
    100% { opacity: 0.98; }
  }

  .terminal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 2px solid #ffb000;
    background: rgba(255, 176, 0, 0.1);
  }

  .terminal-title {
    font-size: 16px;
    font-weight: bold;
    letter-spacing: 2px;
    text-shadow: 0 0 10px rgba(255, 176, 0, 0.8);
  }

  .close-btn {
    background: transparent;
    border: 1px solid #ffb000;
    color: #ffb000;
    padding: 4px 10px;
    cursor: pointer;
    font-family: inherit;
    font-size: 14px;
    transition: all 0.2s;
  }

  .close-btn:hover {
    background: #ffb000;
    color: #1a1000;
  }

  .terminal-body {
    flex: 1;
    overflow-y: auto;
    padding: 15px 20px;
    scrollbar-width: thin;
    scrollbar-color: #ffb000 #1a1000;
  }

  .terminal-body::-webkit-scrollbar {
    width: 8px;
  }

  .terminal-body::-webkit-scrollbar-track {
    background: #1a1000;
  }

  .terminal-body::-webkit-scrollbar-thumb {
    background: #ffb000;
    border-radius: 4px;
  }

  .output-line {
    margin: 0;
    padding: 0;
    font-size: 13px;
    line-height: 1.4;
    white-space: pre-wrap;
    word-break: break-word;
    text-shadow: 0 0 5px rgba(255, 176, 0, 0.5);
  }

  .input-line {
    display: flex;
    align-items: center;
    margin-top: 5px;
    font-size: 13px;
  }

  .prompt {
    color: #ff8c00;
    margin-right: 8px;
    font-weight: bold;
    text-shadow: 0 0 8px rgba(255, 140, 0, 0.8);
  }

  .terminal-input {
    flex: 1;
    background: transparent;
    border: none;
    color: #ffb000;
    font-family: inherit;
    font-size: 13px;
    outline: none;
    caret-color: transparent;
    text-shadow: 0 0 5px rgba(255, 176, 0, 0.5);
  }

  .terminal-input:disabled {
    opacity: 0.7;
  }

  .cursor {
    color: #ffb000;
    font-weight: bold;
    text-shadow: 0 0 10px rgba(255, 176, 0, 0.8);
  }

  .cursor.blink {
    animation: blink 1s step-end infinite;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0; }
  }

  /* Glow effect for the whole terminal */
  .crt-container::after {
    content: '';
    position: absolute;
    inset: -5px;
    background: radial-gradient(
      ellipse at center,
      rgba(255, 176, 0, 0.1) 0%,
      transparent 70%
    );
    pointer-events: none;
    z-index: -1;
  }
</style>
