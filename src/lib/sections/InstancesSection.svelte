<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { confirm } from '@tauri-apps/plugin-dialog';

  interface Instance {
    id: string;
    name: string;
    port: number;
    service_type: string;
    version: string;
    running: boolean;
    pid: number | null;
    healthy: boolean | null;
    has_config: boolean;
    domain: string;
    domain_enabled: boolean;
    process_manager: string;
    stack_id: string | null;
    mapped_domains: string[];
  }

  interface Stack {
    id: string;
    name: string;
    description: string | null;
    created_at: string;
    updated_at: string;
  }

  interface ServiceInfo {
    id: string;
    display_name: string;
    default_port: number;
    config_fields: ConfigFieldInfo[];
    available: boolean;
    is_homebrew: boolean;
    process_manager: string;
  }

  interface ConfigFieldInfo {
    key: string;
    label: string;
    field_type: string;
    required: boolean;
    default: string | null;
  }

  interface BinaryStatus {
    service_type: string;
    installed: boolean;
    version: string | null;
    path: string | null;
  }

  interface DomainInfo {
    id: string;
    subdomain: string;
    target_type: string;
    target_value: string;
    ssl_enabled: boolean;
    full_domain: string;
  }

  let {
    instances = [],
    stacks = [],
    serviceTypes = [],
    binaryStatuses = [],
    installedVersions = {},
    loading = false,
    resolverInstalled = false,
    actionLoading = {},
    initializingInstances = {},
    initializedInstances = {},
    tld = "burd",
    // Event handlers
    onStart,
    onStop,
    onRestart,
    onDelete,
    onCreate,
    onViewLogs,
    onViewEnv,
    onViewInfo,
    onOpenSettings,
    onOpenService,
    onOpenDomain,
    onServiceTypeChange,
    onRefresh,
    onInitialize,
    // Stack event handlers
    onCreateStack,
    onDeleteStack,
    onExportStack,
    onImportStack,
    onStartStack,
    onStopStack,
  }: {
    instances: Instance[];
    stacks: Stack[];
    serviceTypes: ServiceInfo[];
    binaryStatuses: BinaryStatus[];
    installedVersions: Record<string, string[]>;
    loading: boolean;
    resolverInstalled: boolean;
    actionLoading: Record<string, boolean>;
    initializingInstances: Record<string, boolean>;
    initializedInstances: Record<string, boolean>;
    tld?: string;
    onStart: (id: string) => void;
    onStop: (id: string) => void;
    onRestart: (id: string) => void;
    onDelete: (id: string, name: string) => void;
    onCreate: (name: string, port: number, serviceType: string, version: string, config: Record<string, string>, customDomain?: string | null) => void;
    onViewLogs: (id: string, name: string) => void;
    onViewEnv: (id: string, name: string, serviceType: string) => void;
    onViewInfo: (id: string, name: string, serviceType: string) => void;
    onOpenSettings: (instance: Instance) => void;
    onOpenService: (port: number, serviceType: string) => void;
    onOpenDomain: (instance: Instance) => void;
    onServiceTypeChange: (serviceType: string) => { port: number };
    onRefresh: () => void;
    onInitialize: (id: string) => void;
    // Stack event handlers
    onCreateStack?: (name: string, description: string | null, instanceIds: string[]) => void;
    onDeleteStack?: (id: string) => void;
    onExportStack?: (id: string) => void;
    onImportStack?: () => void;
    onStartStack?: (id: string) => void;
    onStopStack?: (id: string) => void;
  } = $props();

  // Mouse-based drag state (HTML5 DnD doesn't work in Tauri WebView)
  let isDragging = $state(false);
  let draggedInstanceId = $state<string | null>(null);
  let dragOverTarget = $state<string | null>(null);
  let errorMessage = $state<string | null>(null);

  // Reordering state
  let dragOverIndex = $state<number | null>(null);
  let draggedFromIndex = $state<number | null>(null);

  // Stack drag state
  let dragOverInstanceId = $state<string | null>(null);

  // Mouse-based drag-and-drop implementation
  $effect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      // Find which row we're over
      const elements = document.elementsFromPoint(e.clientX, e.clientY);
      const gridRow = elements.find(el => el.classList.contains('grid-row')) as HTMLElement;

      if (gridRow) {
        const rowIndex = gridRow.getAttribute('data-index');
        const instanceId = gridRow.getAttribute('data-instance-id');

        if (instanceId && instanceId !== draggedInstanceId) {
          const instance = instances.find(i => i.id === instanceId);

          if (instance) {
            dragOverInstanceId = instanceId;

            // For standalone instances, also track index for reordering
            if (!instance.stack_id && rowIndex) {
              dragOverIndex = parseInt(rowIndex, 10);
            } else {
              dragOverIndex = null;
            }
          }
        }
      } else {
        dragOverIndex = null;
        dragOverInstanceId = null;
      }
    };

    const handleMouseUp = async (e: MouseEvent) => {
      if (!draggedInstanceId) {
        cleanupDrag();
        return;
      }

      const draggedInstance = instances.find(i => i.id === draggedInstanceId);
      const targetInstance = dragOverInstanceId ? instances.find(i => i.id === dragOverInstanceId) : null;

      if (!draggedInstance) {
        cleanupDrag();
        return;
      }

      try {
        // Determine what operation to perform
        if (targetInstance) {
          const sourceStackId = draggedInstance.stack_id;
          const targetStackId = targetInstance.stack_id;

          if (sourceStackId === targetStackId) {
            // Same context (both standalone or same stack)
            if (!sourceStackId && dragOverIndex !== null && draggedFromIndex !== null && dragOverIndex !== draggedFromIndex) {
              // Reorder within standalone instances
              await handleRowReorder();
            }
            // If same stack, we don't support reordering within stacks yet
          } else {
            // Different contexts - move between stacks or to/from standalone
            if (targetStackId) {
              // Moving TO a stack
              await invoke('move_instance_to_stack', {
                instanceId: draggedInstanceId,
                stackId: targetStackId,
              });
              onRefresh();
            } else {
              // Moving to standalone (removing from stack)
              await invoke('remove_instances_from_stack', {
                instanceIds: [draggedInstanceId],
              });
              onRefresh();
            }
          }
        }
      } catch (error) {
        console.error('❌ Drag operation error:', error);
        errorMessage = `Failed to move instance: ${error}`;
        setTimeout(() => { errorMessage = null; }, 5000);
      }

      cleanupDrag();
    };

    function cleanupDrag() {
      document.body.classList.remove('dragging-instance');
      isDragging = false;
      draggedInstanceId = null;
      dragOverIndex = null;
      draggedFromIndex = null;
      dragOverInstanceId = null;
    }

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  });

  // Mouse-based drag start (replaces HTML5 dragstart)
  function handleMouseDragStart(event: MouseEvent, instanceId: string) {
    // Only start drag on left mouse button
    if (event.button !== 0) return;

    event.preventDefault(); // Prevent text selection

    isDragging = true;
    draggedInstanceId = instanceId;
    document.body.classList.add('dragging-instance');

    // Track the index for reordering standalone instances
    const standaloneInstances = getStandaloneInstances();
    draggedFromIndex = standaloneInstances.findIndex(i => i.id === instanceId);
  }

  // Handle the actual reorder operation
  async function handleRowReorder() {
    if (draggedFromIndex === null || dragOverIndex === null) return;
    if (draggedFromIndex === dragOverIndex) return;

    try {
      const standaloneInstances = getStandaloneInstances();

      // Create new order by moving the dragged item
      const newOrder = [...standaloneInstances];
      const [draggedInstance] = newOrder.splice(draggedFromIndex, 1);
      newOrder.splice(dragOverIndex, 0, draggedInstance);

      // Get ordered list of IDs
      const orderedIds = newOrder.map(i => i.id);

      // Call backend with the new full order
      await invoke('reorder_instances', {
        instanceIds: orderedIds
      });

      onRefresh();
    } catch (error) {
      console.error('Failed to reorder instances:', error);
      errorMessage = `Failed to reorder: ${error}`;
      setTimeout(() => { errorMessage = null; }, 5000);
    }
  }

  // HTML5 drag-and-drop handlers (fallback, used in template ondragover/ondragleave/ondrop)
  function handleDragOver(e: DragEvent, target: string) {
    e.preventDefault();
    dragOverTarget = target;
  }

  function handleDragLeave(_e: DragEvent) {
    dragOverTarget = null;
  }

  async function handleDrop(e: DragEvent, targetType: string, stackId?: string) {
    e.preventDefault();
    dragOverTarget = null;

    if (!draggedInstanceId) return;

    try {
      if (targetType === 'standalone') {
        await invoke('remove_instances_from_stack', {
          instanceIds: [draggedInstanceId],
        });
        onRefresh();
      } else if (targetType === 'stack' && stackId) {
        await invoke('move_instance_to_stack', {
          instanceId: draggedInstanceId,
          stackId: stackId,
        });
        onRefresh();
      }
    } catch (error) {
      console.error('Drag-drop operation error:', error);
      errorMessage = `Failed to move instance: ${error}`;
      setTimeout(() => { errorMessage = null; }, 5000);
    }
  }

  // Service icons from simple-icons (hardcoded for ESM compatibility)
  const serviceStyles: Record<string, { color: string; icon: string }> = {
    meilisearch: {
      color: "#FF5CAA",
      icon: `<path d="m6.505 18.998 4.434-11.345a4.168 4.168 0 0 1 3.882-2.651h2.674l-4.434 11.345a4.169 4.169 0 0 1-3.883 2.651H6.505Zm6.505 0 4.434-11.345a4.169 4.169 0 0 1 3.883-2.651H24l-4.434 11.345a4.168 4.168 0 0 1-3.882 2.651H13.01Zm-13.01 0L4.434 7.653a4.168 4.168 0 0 1 3.882-2.651h2.674L6.556 16.347a4.169 4.169 0 0 1-3.883 2.651H0Z"/>`
    },
    typesense: {
      color: "#D52E63",
      icon: `<path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>`
    },
    mongodb: {
      color: "#47A248",
      icon: `<path d="M17.193 9.555c-1.264-5.58-4.252-7.414-4.573-8.115-.28-.394-.53-.954-.735-1.44-.036.495-.055.685-.523 1.184-.723.566-4.438 3.682-4.74 10.02-.282 5.912 4.27 9.435 4.888 9.884l.07.05A73.49 73.49 0 0111.91 24h.481c.114-1.032.284-2.056.51-3.07.417-.296.604-.463.85-.693a11.342 11.342 0 003.639-8.464c.01-.814-.103-1.662-.197-2.218zm-5.336 8.195s0-8.291.275-8.29c.213 0 .49 10.695.49 10.695-.381-.045-.765-1.76-.765-2.405z"/>`
    },
    minio: {
      color: "#C72E49",
      icon: `<path d="M13.2072.006c-.6216-.0478-1.2.1943-1.6211.582a2.15 2.15 0 0 0-.0938 3.0352l3.4082 3.5507a3.042 3.042 0 0 1-.664 4.6875l-.463.2383V7.2853a15.4198 15.4198 0 0 0-8.0174 10.4862v.0176l6.5487-3.3281v7.621L13.7794 24V13.6817l.8965-.4629a4.4432 4.4432 0 0 0 1.2207-7.0292l-3.371-3.5254a.7489.7489 0 0 1 .037-1.0547.7522.7522 0 0 1 1.0567.0371l.4668.4863-.006.0059 4.0704 4.2441a.0566.0566 0 0 0 .082 0 .06.06 0 0 0 0-.0703l-3.1406-5.1425-.1484.1425.1484-.1445C14.4945.3926 13.8287.0538 13.2072.006Zm-.9024 9.8652v2.9941l-4.1523 2.1484a13.9787 13.9787 0 0 1 2.7676-3.9277 14.1784 14.1784 0 0 1 1.3847-1.2148z"/>`
    },
    frankenphp: {
      color: "#777BB4",
      icon: `<path d="M7.01 10.207h-.944l-.515 2.648h.838c.556 0 .97-.105 1.242-.314.272-.21.455-.559.55-1.049.092-.47.05-.802-.124-.995-.175-.193-.523-.29-1.047-.29zM12 5.688C5.373 5.688 0 8.514 0 12s5.373 6.313 12 6.313S24 15.486 24 12c0-3.486-5.373-6.312-12-6.312zm-3.26 7.451c-.261.25-.575.438-.917.551-.336.108-.765.164-1.285.164H5.357l-.327 1.681H3.652l1.23-6.326h2.65c.797 0 1.378.209 1.744.628.366.418.476 1.002.33 1.752a2.836 2.836 0 0 1-.305.847c-.143.255-.33.49-.561.703zm4.024.715l.543-2.799c.063-.318.039-.536-.068-.651-.107-.116-.336-.174-.687-.174H11.46l-.704 3.625H9.388l1.23-6.327h1.367l-.327 1.682h1.218c.767 0 1.295.134 1.586.401s.378.7.263 1.299l-.572 2.944h-1.389zm7.597-2.265a2.782 2.782 0 0 1-.305.847c-.143.255-.33.49-.561.703a2.44 2.44 0 0 1-.917.551c-.336.108-.765.164-1.286.164h-1.18l-.327 1.682h-1.378l1.23-6.326h2.649c.797 0 1.378.209 1.744.628.366.417.477 1.001.331 1.751zM17.766 10.207h-.943l-.516 2.648h.838c.557 0 .971-.105 1.242-.314.272-.21.455-.559.551-1.049.092-.47.049-.802-.125-.995s-.524-.29-1.047-.29z"/>`
    },
    mariadb: {
      color: "#003545",
      icon: `<path d="M23.157 4.412c-.676.284-.79.31-1.673.372-.65.045-.757.057-1.212.209-.75.246-1.395.75-2.02 1.59-.296.398-1.249 1.913-1.249 1.988 0 .057-.65.998-.915 1.32-.574.713-1.08 1.079-2.14 1.59-.77.36-1.224.524-4.102 1.477-1.073.353-2.133.738-2.367.864-.852.449-1.515 1.036-2.203 1.938-1.003 1.32-.972 1.313-3.042.947a12.264 12.264 0 00-.675-.063c-.644-.05-1.023.044-1.332.334L0 17.193l.177.088c.094.05.353.234.561.398.215.17.461.347.55.391.088.044.17.088.183.101.012.013-.089.17-.228.353-.435.581-.593.871-.574 1.048.019.164.032.17.43.17.517-.006.826-.056 1.261-.208.65-.233 2.058-.94 2.784-1.4.776-.5 1.717-.998 1.956-1.042.082-.02.354-.07.594-.114.58-.107 1.464-.095 2.587.05.108.013.373.045.6.064.227.025.43.057.454.076.026.012.474.037.998.056.934.026 1.104.007 1.3-.189.126-.133.385-.631.498-.985.209-.643.417-.921.366-.492-.113.966-.322 1.692-.713 2.411-.259.499-.663 1.092-.934 1.395-.322.347-.315.36.088.315.619-.063 1.471-.397 2.096-.82.827-.562 1.647-1.691 2.19-3.03.107-.27.22-.22.183.083-.013.094-.038.315-.057.498l-.031.328.353-.202c.833-.48 1.414-1.262 2.127-2.884.227-.518.877-2.922 1.073-3.976a9.64 9.64 0 01.271-1.042c.127-.429.196-.555.48-.858.183-.19.625-.555.978-.808.72-.505.953-.75 1.187-1.205.208-.417.284-1.13.132-1.357-.132-.202-.284-.196-.763.006Z"/>`
    },
    postgresql: {
      color: "#4169E1",
      icon: `<path d="M23.5594 14.7228a.5269.5269 0 0 0-.0563-.1191c-.139-.2632-.4768-.3418-1.0074-.2321-1.6533.3411-2.2935.1312-2.5256-.0191 1.342-2.0482 2.445-4.522 3.0411-6.8297.2714-1.0507.7982-3.5237.1222-4.7316a1.5641 1.5641 0 0 0-.1509-.235C21.6931.9086 19.8007.0248 17.5099.0005c-1.4947-.0158-2.7705.3461-3.1161.4794a9.449 9.449 0 0 0-.5159-.0816 8.044 8.044 0 0 0-1.3114-.1278c-1.1822-.0184-2.2038.2642-3.0498.8406-.8573-.3211-4.7888-1.645-7.2219.0788C.9359 2.1526.3086 3.8733.4302 6.3043c.0409.818.5069 3.334 1.2423 5.7436.4598 1.5065.9387 2.7019 1.4334 3.582.553.9942 1.1259 1.5933 1.7143 1.7895.4474.1491 1.1327.1441 1.8581-.7279.8012-.9635 1.5903-1.8258 1.9446-2.2069.4351.2355.9064.3625 1.39.3772a.0569.0569 0 0 0 .0004.0041 11.0312 11.0312 0 0 0-.2472.3054c-.3389.4302-.4094.5197-1.5002.7443-.3102.064-1.1344.2339-1.1464.8115-.0025.1224.0329.2309.0919.3268.2269.4231.9216.6097 1.015.6331 1.3345.3335 2.5044.092 3.3714-.6787-.017 2.231.0775 4.4174.3454 5.0874.2212.5529.7618 1.9045 2.4692 1.9043.2505 0 .5263-.0291.8296-.0941 1.7819-.3821 2.5557-1.1696 2.855-2.9059.1503-.8707.4016-2.8753.5388-4.1012.0169-.0703.0357-.1207.057-.1362.0007-.0005.0697-.0471.4272.0307a.3673.3673 0 0 0 .0443.0068l.2539.0223.0149.001c.8468.0384 1.9114-.1426 2.5312-.4308.6438-.2988 1.8057-1.0323 1.5951-1.6698z"/>`
    },
    redis: {
      color: "#FF4438",
      icon: `<path d="M22.71 13.145c-1.66 2.092-3.452 4.483-7.038 4.483-3.203 0-4.397-2.825-4.48-5.12.701 1.484 2.073 2.685 4.214 2.63 4.117-.133 6.94-3.852 6.94-7.239 0-4.05-3.022-6.972-8.268-6.972-3.752 0-8.4 1.428-11.455 3.685C2.59 6.937 3.885 9.958 4.35 9.626c2.648-1.904 4.748-3.13 6.784-3.744C8.12 9.244.886 17.05 0 18.425c.1 1.261 1.66 4.648 2.424 4.648.232 0 .431-.133.664-.365a100.49 100.49 0 0 0 5.54-6.765c.222 3.104 1.748 6.898 6.014 6.898 3.819 0 7.604-2.756 9.33-8.965.2-.764-.73-1.361-1.261-.73zm-4.349-5.013c0 1.959-1.926 2.922-3.685 2.922-.941 0-1.664-.247-2.235-.568 1.051-1.592 2.092-3.225 3.21-4.973 1.972.334 2.71 1.43 2.71 2.619z"/>`
    },
    valkey: {
      color: "#00A89D",
      icon: `<path d="M12 2L3 7v10l9 5 9-5V7l-9-5zm0 2.18l6.37 3.53L12 11.24 5.63 7.71 12 4.18zM5 9.23l6 3.33v6.22l-6-3.33V9.23zm8 9.55v-6.22l6-3.33v6.22l-6 3.33z"/>`
    },
    mailpit: {
      color: "#0891b2",
      icon: `<path d="M20 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"/>`
    },
    beanstalkd: {
      color: "#22c55e",
      icon: `<path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>`
    },
    memcached: {
      color: "#059669",
      icon: `<path d="M2 20h20v-4H2v4zm2-3h2v2H4v-2zM2 4v4h20V4H2zm4 3H4V5h2v2zm-4 7h20v-4H2v4zm2-3h2v2H4v-2z"/>`
    },
    frpc: {
      color: "#00ADD8",
      icon: `<path d="M1.811 10.231c-.047 0-.058-.023-.035-.059l.246-.315c.023-.035.081-.058.128-.058h4.172c.046 0 .058.035.035.07l-.199.303c-.023.036-.082.07-.117.07zM.047 11.306c-.047 0-.059-.023-.035-.058l.245-.316c.023-.035.082-.058.129-.058h5.328c.047 0 .07.035.058.07l-.093.28c-.012.047-.058.07-.105.07zm2.828 1.075c-.047 0-.059-.035-.035-.07l.163-.292c.023-.035.07-.07.117-.07h2.337c.047 0 .07.035.07.082l-.023.28c0 .047-.047.082-.082.082zm12.129-2.36c-.736.187-1.239.327-1.963.514-.176.046-.187.058-.34-.117-.174-.199-.303-.327-.548-.444-.737-.362-1.45-.257-2.115.175-.795.514-1.204 1.274-1.192 2.22.011.935.654 1.706 1.577 1.835.795.105 1.46-.175 1.987-.77.105-.13.198-.27.315-.434H10.47c-.245 0-.304-.152-.222-.35.152-.362.432-.97.596-1.274a.315.315 0 01.292-.187h4.253c-.023.316-.023.631-.07.947a4.983 4.983 0 01-.958 2.29c-.841 1.11-1.94 1.8-3.33 1.986-1.145.152-2.209-.07-3.143-.77-.865-.655-1.356-1.52-1.484-2.595-.152-1.274.222-2.419.993-3.424.83-1.086 1.928-1.776 3.272-2.02 1.098-.2 2.15-.07 3.096.571.62.41 1.063.97 1.356 1.648.07.105.023.164-.117.2m3.868 6.461c-1.064-.024-2.034-.328-2.852-1.029a3.665 3.665 0 01-1.262-2.255c-.21-1.32.152-2.489.947-3.529.853-1.122 1.881-1.706 3.272-1.95 1.192-.21 2.314-.095 3.33.595.923.63 1.496 1.484 1.648 2.605.198 1.578-.257 2.863-1.344 3.962-.771.783-1.718 1.273-2.805 1.495-.315.06-.63.07-.934.106zm2.78-4.72c-.011-.153-.011-.27-.034-.387-.21-1.157-1.274-1.81-2.384-1.554-1.087.245-1.788.935-2.045 2.033-.21.912.234 1.835 1.075 2.21.643.28 1.285.244 1.905-.07.923-.48 1.425-1.228 1.484-2.233z"/>`
    },
    nodered: {
      color: "#8F0000",
      icon: `<path d="M3 0C1.338 0 0 1.338 0 3v6.107h2.858c1.092 0 1.97.868 1.964 1.96v.021c.812-.095 1.312-.352 1.674-.683.416-.382.69-.91 1.016-1.499.325-.59.71-1.244 1.408-1.723.575-.395 1.355-.644 2.384-.686v-.45c0-1.092.88-1.976 1.972-1.976h7.893c1.091 0 1.974.884 1.974 1.976v1.942c0 1.091-.883 2.029-1.974 2.029h-7.893c-1.092 0-1.972-.938-1.972-2.03v-.453c-.853.037-1.408.236-1.798.504-.48.33-.774.802-1.086 1.368-.312.565-.63 1.22-1.222 1.763l-.077.069c3.071.415 4.465 1.555 5.651 2.593 1.39 1.215 2.476 2.275 6.3 2.288v-.46c0-1.092.894-1.946 1.986-1.946H24V3c0-1.662-1.338-3-3-3zm10.276 5.41c-.369 0-.687.268-.687.637v1.942c0 .368.318.636.687.636h7.892a.614.614 0 0 0 .635-.636V6.047a.614.614 0 0 0-.635-.636zM0 10.448v3.267h2.858a.696.696 0 0 0 .678-.69v-1.942c0-.368-.31-.635-.678-.635zm4.821 1.67v.907A1.965 1.965 0 0 1 2.858 15H0v6c0 1.662 1.338 3 3 3h18c1.662 0 3-1.338 3-3v-1.393h-2.942c-1.092 0-1.986-.913-1.986-2.005v-.445c-4.046-.032-5.598-1.333-6.983-2.544-1.437-1.257-2.751-2.431-7.268-2.496zM21.058 15a.644.644 0 0 0-.647.66v1.942c0 .368.278.612.647.612H24V15z"/>`
    },
    caddy: {
      color: "#1F88C0",
      icon: `<path d="M11.094.47c-.842 0-1.696.092-2.552.288a11.37 11.37 0 0 0-4.87 2.423 10.632 10.632 0 0 0-2.36 2.826A10.132 10.132 0 0 0 .305 8.582c-.398 1.62-.4 3.336-.043 5.048.085.405.183.809.31 1.212a11.85 11.85 0 0 0 1.662 3.729 3.273 3.273 0 0 0-.086.427 3.323 3.323 0 0 0 2.848 3.71 3.279 3.279 0 0 0 1.947-.346c1.045.51 2.17.864 3.339 1.04a11.66 11.66 0 0 0 4.285-.155 11.566 11.566 0 0 0 4.936-2.485 10.643 10.643 0 0 0 2.352-2.894 11.164 11.164 0 0 0 1.356-4.424 11.214 11.214 0 0 0-.498-4.335c.175-.077.338-.175.486-.293a.444.444 89.992 0 0 .001 0c.402-.322.693-.794.777-1.342a2.146 2.146 0 0 0-1.79-2.434 2.115 2.115 0 0 0-1.205.171c-.038-.043-.078-.086-.113-.13a11.693 11.693 0 0 0-3.476-2.93 13.348 13.348 0 0 0-1.76-.81 13.55 13.55 0 0 0-2.06-.613A12.121 12.121 0 0 0 11.093.47Zm.714.328c.345-.004.688.01 1.028.042a9.892 9.892 0 0 1 2.743.639c.984.39 1.89.958 2.707 1.632.803.662 1.502 1.45 2.091 2.328.026.039.048.08.07.12a2.12 2.12 0 0 0-.435 2.646c-.158.114-.97.692-1.634 1.183-.414.308-.733.557-.733.557l.581.68s.296-.276.665-.638c.572-.562 1.229-1.233 1.395-1.403a2.122 2.122 0 0 0 1.907.677 11.229 11.229 0 0 1-.013 4.046 11.41 11.41 0 0 1-1.475 3.897 12.343 12.343 0 0 1-2.079 2.587c-1.19 1.125-2.633 2.022-4.306 2.531a10.826 10.826 0 0 1-3.973.484 11.04 11.04 0 0 1-3.057-.652 3.304 3.304 0 0 0 1.417-2.294 3.275 3.275 0 0 0-.294-1.842c.18-.162.403-.363.656-.6 1.015-.955 2.353-2.303 2.353-2.303l-.47-.599s-1.63.972-2.801 1.728c-.307.198-.573.378-.777.517a3.273 3.273 0 0 0-1.516-.611c-1.507-.198-2.927.672-3.487 2.017a10.323 10.323 0 0 1-.695-1.078A10.92 10.92 0 0 1 .728 14.8a10.35 10.35 0 0 1-.2-1.212c-.164-1.653.103-3.258.629-4.754a12.95 12.95 0 0 1 1.087-2.288c.57-.968 1.248-1.872 2.069-2.656A11.013 11.013 0 0 1 11.808.797Zm-.147 3.257a3.838 3.838 0 0 0-3.82 3.82v2.36h-.94c-.751 0-1.377.625-1.377 1.377v3.8h1.46v-3.718h9.354v6.264H10.02v1.46h6.4c.751 0 1.377-.625 1.377-1.377v-6.43c0-.751-.626-1.377-1.377-1.377h-.94v-2.36a3.838 3.838 0 0 0-3.82-3.819zm0 1.46a2.371 2.371 0 0 1 2.36 2.36v2.36H9.3v-2.36a2.372 2.372 0 0 1 2.36-2.36zm10.141.392a1.253 1.253 0 0 1 1.296 1.434c-.049.319-.217.59-.453.78-.266.213-.61.318-.968.264a1.253 1.253 0 0 1-1.045-1.42 1.255 1.255 0 0 1 1.17-1.058zM5.384 17.425a2.02 2.02 0 0 1 1.917 1.298c.116.3.159.628.114.967a2.015 2.015 0 0 1-2.249 1.728 2.016 2.016 0 0 1-1.727-2.25 2.017 2.017 0 0 1 1.945-1.743z"/>`
    }
  };

  function getServiceStyle(serviceId: string) {
    // Normalize: "Meilisearch" -> "meilisearch", "frp Client" -> "frpc", "Node-RED" -> "nodered"
    const normalized = serviceId.toLowerCase().replace(/[\s-]+/g, '').replace('frpclient', 'frpc');
    return serviceStyles[normalized] || { color: '#6b7280', icon: '<circle cx="12" cy="12" r="10"/>' };
  }

  // Local form state
  let showCreateForm = $state(false);
  let copiedId = $state<string | null>(null);

  // Stack UI state
  let collapsedStacks = $state<Set<string>>(new Set());
  let selectionMode = $state(false);
  let selectedInstances = $state<Set<string>>(new Set());
  let showCreateStackModal = $state(false);
  let newStackName = $state("");
  let newStackDescription = $state("");

  // Toggle stack collapse
  function toggleStackCollapse(stackId: string) {
    const newSet = new Set(collapsedStacks);
    if (newSet.has(stackId)) {
      newSet.delete(stackId);
    } else {
      newSet.add(stackId);
    }
    collapsedStacks = newSet;
  }

  // Toggle instance selection
  function toggleInstanceSelection(instanceId: string) {
    const newSet = new Set(selectedInstances);
    if (newSet.has(instanceId)) {
      newSet.delete(instanceId);
    } else {
      newSet.add(instanceId);
    }
    selectedInstances = newSet;
  }

  // Get instances for a specific stack
  function getInstancesForStack(stackId: string): Instance[] {
    return instances.filter(i => i.stack_id === stackId);
  }

  // Get standalone instances (not in any stack)
  function getStandaloneInstances(): Instance[] {
    return instances.filter(i => !i.stack_id);
  }

  // Get running count for a stack
  function getStackRunningCount(stackId: string): number {
    return getInstancesForStack(stackId).filter(i => i.running).length;
  }

  // Check if all instances in stack are running
  function isStackFullyRunning(stackId: string): boolean {
    const stackInstances = getInstancesForStack(stackId);
    return stackInstances.length > 0 && stackInstances.every(i => i.running);
  }

  // Cancel selection mode
  function cancelSelection() {
    selectionMode = false;
    selectedInstances = new Set();
  }

  // Open create stack modal
  function openCreateStackModal() {
    if (selectedInstances.size > 0) {
      showCreateStackModal = true;
    }
  }

  // Handle create stack
  function handleCreateStack() {
    if (onCreateStack && newStackName.trim() && selectedInstances.size > 0) {
      onCreateStack(
        newStackName.trim(),
        newStackDescription.trim() || null,
        Array.from(selectedInstances)
      );
      // Reset state
      showCreateStackModal = false;
      newStackName = "";
      newStackDescription = "";
      cancelSelection();
    }
  }

  async function copyToClipboard(text: string, id: string) {
    await navigator.clipboard.writeText(text);
    copiedId = id;
    setTimeout(() => {
      if (copiedId === id) copiedId = null;
    }, 2000);
  }

  async function handleDeleteDomain(fullDomain: string, instanceId: string) {
    try {
      // Confirm deletion
      const confirmed = await confirm(`Are you sure you want to remove domain "${fullDomain}"?\n\nThis will unregister the domain from the proxy and delete it from the configuration.`, {
        title: 'Burd',
        kind: 'warning'
      });

      if (!confirmed) return;

      // Extract subdomain from full domain (remove TLD)
      const subdomain = fullDomain.split('.')[0];

      // Find the domain ID by subdomain
      const domains = await invoke<DomainInfo[]>("list_domains");
      const domainToDelete = domains.find(d =>
        d.subdomain === subdomain &&
        d.target_type === "instance" &&
        d.target_value === instanceId
      );

      if (!domainToDelete) {
        alert(`Domain "${fullDomain}" not found`);
        return;
      }

      // Delete the domain
      await invoke("delete_domain", { id: domainToDelete.id });

      // Refresh instances list
      await onRefresh();

    } catch (err) {
      alert(`Failed to delete domain: ${err}`);
    }
  }

  let newName = $state("");
  let newServiceType = $state("");
  let newPort = $state(7700);
  let newVersion = $state("");
  let newConfigValue = $state("");
  let newDomain = $state("");  // Custom domain subdomain
  let creating = $state(false);

  // Initialize form with first service type
  $effect(() => {
    if (serviceTypes.length > 0 && !newServiceType) {
      newServiceType = serviceTypes[0].id;
      newPort = serviceTypes[0].default_port;
    }
  });

  // Update version when service type or installed versions change
  $effect(() => {
    const versions = installedVersions[newServiceType] || [];
    if (versions.length > 0 && !versions.includes(newVersion)) {
      newVersion = versions[0];
    }
  });

  function getSelectedServiceMeta(): ServiceInfo | undefined {
    return serviceTypes.find(s => s.id === newServiceType);
  }

  function getBinaryStatus(serviceType: string): BinaryStatus | undefined {
    return binaryStatuses.find(b => b.service_type === serviceType);
  }

  function hasWebUI(serviceType: string): boolean {
    return ['meilisearch', 'frankenphp', 'minio'].includes(serviceType.toLowerCase());
  }

  function handleServiceTypeChange() {
    const result = onServiceTypeChange(newServiceType);
    newPort = result.port;
    newConfigValue = "";
    // Update version
    const versions = installedVersions[newServiceType] || [];
    newVersion = versions.length > 0 ? versions[0] : "";
  }

  async function handleCreate() {
    // Validate subdomain format if provided
    if (newDomain && !/^[a-z0-9.-]+$/.test(newDomain)) {
      alert("Domain must contain only lowercase letters, numbers, hyphens, and periods");
      return;
    }

    creating = true;
    try {
      const configObj: Record<string, string> = {};
      const meta = getSelectedServiceMeta();
      if (meta?.config_fields?.[0] && newConfigValue) {
        configObj[meta.config_fields[0].key] = newConfigValue;
      }
      await onCreate(newName, newPort, newServiceType, newVersion, configObj, newDomain || null);
      // Reset form
      newName = "";
      newConfigValue = "";
      newDomain = "";
      showCreateForm = false;
    } finally {
      creating = false;
    }
  }

  function hasInstalledVersions(): boolean {
    return binaryStatuses.some(b => b.installed);
  }

  function isNodeRed(serviceType: string): boolean {
    const lower = serviceType.toLowerCase();
    return lower === 'nodered' || lower === 'node-red';
  }

  function isPm2Managed(instance: Instance): boolean {
    return instance.process_manager === 'pm2';
  }

  function needsInitialization(instance: Instance): boolean {
    if (!isNodeRed(instance.service_type)) return false;
    // If we don't have status yet, assume needs initialization
    if (initializedInstances[instance.id] === undefined) return true;
    return !initializedInstances[instance.id];
  }

  // Get the reason why the start button is disabled (if any)
  function getStartDisabledReason(instance: Instance): string | null {
    if (actionLoading[instance.id]) {
      return "Action in progress...";
    }
    if (!isNodeRed(instance.service_type)) {
      const status = getBinaryStatus(instance.service_type);
      if (!status?.installed) {
        const serviceName = serviceTypes.find(s => s.id === instance.service_type)?.display_name || instance.service_type;
        return `${serviceName} binary not installed. Go to Services section to download it.`;
      }
    }
    return null;
  }

  // Check if start button should be disabled
  function isStartDisabled(instance: Instance): boolean {
    return actionLoading[instance.id] || (!isNodeRed(instance.service_type) && !getBinaryStatus(instance.service_type)?.installed);
  }
</script>

<div class="instances-section">
  <div class="section-header">
    <div class="title-row">
      <h2>Instances</h2>
      <button class="refresh-btn" onclick={onRefresh} title="Refresh">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 2v6h-6"></path>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
          <path d="M3 22v-6h6"></path>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
        </svg>
      </button>
    </div>
    <div class="header-actions">
      {#if instances.length >= 2 && !selectionMode}
        <button
          class="btn secondary small"
          onclick={() => selectionMode = true}
          title="Select instances to group into a stack"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="3" width="7" height="7"></rect>
            <rect x="14" y="3" width="7" height="7"></rect>
            <rect x="14" y="14" width="7" height="7"></rect>
            <rect x="3" y="14" width="7" height="7"></rect>
          </svg>
          Create Stack
        </button>
      {/if}
      {#if onImportStack}
        <button
          class="btn secondary small"
          onclick={onImportStack}
          title="Import a stack configuration"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
          </svg>
          Import Stack
        </button>
      {/if}
      <button
        class="btn primary small"
        onclick={() => (showCreateForm = !showCreateForm)}
        disabled={!hasInstalledVersions()}
      >
        {showCreateForm ? "Cancel" : "+ New Instance"}
      </button>
    </div>
  </div>

  {#if showCreateForm}
    <form class="create-form" onsubmit={(e) => { e.preventDefault(); handleCreate(); }}>
      <div class="form-row">
        <label>
          <span>Service Type</span>
          <select bind:value={newServiceType} onchange={handleServiceTypeChange}>
            {#each serviceTypes as svc (svc.id)}
              <option value={svc.id}>{svc.display_name}</option>
            {/each}
          </select>
        </label>
        <label>
          <span>Version</span>
          {#if installedVersions[newServiceType]?.length > 0}
            <select bind:value={newVersion}>
              {#each installedVersions[newServiceType] as ver (ver)}
                <option value={ver}>{ver}</option>
              {/each}
            </select>
          {:else}
            <select disabled>
              <option>No versions installed</option>
            </select>
          {/if}
        </label>
        <label>
          <span>Name</span>
          <input
            type="text"
            bind:value={newName}
            placeholder="Development"
            required
          />
        </label>
        <label>
          <span>Domain <span class="optional-label">(optional)</span></span>
          <div class="domain-input-group">
            <input
              type="text"
              bind:value={newDomain}
              placeholder="my-app"
              class="domain-input"
              pattern="[a-z0-9-]+"
            />
            <span class="domain-suffix">.{tld}</span>
          </div>
          <span class="field-hint">
            Leave empty to use instance name. Only lowercase letters, numbers, hyphens, and periods.
          </span>
        </label>
        <label>
          <span>Port</span>
          <input
            type="number"
            bind:value={newPort}
            min="1024"
            max="65535"
            required
          />
        </label>
        {#if getSelectedServiceMeta()?.config_fields?.[0]}
          <label>
            <span>{getSelectedServiceMeta()?.config_fields[0].label} (optional)</span>
            <input
              type={getSelectedServiceMeta()?.config_fields[0].field_type === 'password' ? 'password' : 'text'}
              bind:value={newConfigValue}
              placeholder="Leave empty for default"
            />
          </label>
        {/if}
      </div>
      <button class="btn primary" type="submit" disabled={creating || !newVersion || installedVersions[newServiceType]?.length === 0}>
        {creating ? "Creating..." : "Create Instance"}
      </button>
      {#if installedVersions[newServiceType]?.length === 0}
        <p class="form-hint">Download a {getSelectedServiceMeta()?.display_name} version first.</p>
      {/if}
    </form>
  {/if}

  <!-- Selection mode floating bar -->
  {#if selectionMode}
    <div class="selection-bar">
      <span class="selection-count">{selectedInstances.size} selected</span>
      <div class="selection-actions">
        <button class="btn secondary small" onclick={cancelSelection}>Cancel</button>
        <button
          class="btn primary small"
          onclick={openCreateStackModal}
          disabled={selectedInstances.size === 0}
        >
          Create Stack from Selected
        </button>
      </div>
    </div>
  {/if}

  <!-- Create Stack Modal -->
  {#if showCreateStackModal}
    <div class="modal-overlay" onclick={() => showCreateStackModal = false} onkeydown={(e) => e.key === 'Escape' && (showCreateStackModal = false)} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
        <div class="modal-header">
          <h3>Create Stack</h3>
          <button class="close-btn" onclick={() => showCreateStackModal = false}>&times;</button>
        </div>
        <div class="modal-body">
          <label>
            <span>Stack Name</span>
            <input type="text" bind:value={newStackName} placeholder="My Project Stack" required />
          </label>
          <label>
            <span>Description (optional)</span>
            <textarea bind:value={newStackDescription} placeholder="Backend services for..."></textarea>
          </label>
          <div class="selected-instances-preview">
            <span>Selected instances:</span>
            <ul>
              {#each Array.from(selectedInstances) as instanceId}
                {@const inst = instances.find(i => i.id === instanceId)}
                {#if inst}
                  <li>{inst.name} ({inst.service_type})</li>
                {/if}
              {/each}
            </ul>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn secondary" onclick={() => showCreateStackModal = false}>Cancel</button>
          <button class="btn primary" onclick={handleCreateStack} disabled={!newStackName.trim()}>Create Stack</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Error notification for drag-drop -->
  {#if errorMessage}
    <div class="dnd-error">
      <span>{errorMessage}</span>
      <button class="close-btn" onclick={() => errorMessage = null}>&times;</button>
    </div>
  {/if}

  <!-- Standalone drop zone (visible when dragging instance from stack) -->
  {#if isDragging && draggedInstanceId}
    {@const draggedInstance = instances.find(i => i.id === draggedInstanceId)}
    {#if draggedInstance?.stack_id}
      <div
        class="standalone-drop-zone"
        role="region"
        aria-label="Drop zone to move instance to standalone"
        class:drop-target-active={dragOverTarget === 'standalone-zone'}
        ondragover={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, 'standalone-zone');
        }}
        ondragenter={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, 'standalone-zone');
        }}
        ondragleave={handleDragLeave}
        ondrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDrop(e, 'standalone');
        }}
      >
        <div class="drop-zone-hint">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 3h18v18H3z"></path>
          </svg>
          Drop here to remove from stack
        </div>
      </div>
    {/if}
  {/if}

    <section class="card">
      {#if loading && instances.length === 0}
        <p class="loading">Loading instances...</p>
      {:else if instances.length === 0}
        <p class="empty">No instances yet. Create one to get started.</p>
      {:else}
        {@const standaloneInstances = getStandaloneInstances()}

        <!-- Main grid -->
        <div class="instances-grid">
          <div class="grid-body">
            {#each standaloneInstances as instance, index (instance.id)}
              {@render instanceRow(instance, true, index)}
            {/each}
          </div>
        </div>
      {/if}
    </section>

    <!-- Stacks -->
    {#each stacks as stack (stack.id)}
    {@const stackInstances = getInstancesForStack(stack.id)}
    {@const runningCount = getStackRunningCount(stack.id)}
    {@const isCollapsed = collapsedStacks.has(stack.id)}
    <div class="stack-group">
      <div
        class="stack-header"
        role="button"
        tabindex="0"
        class:drop-target-active={dragOverTarget === `stack-${stack.id}`}
        ondragover={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, `stack-${stack.id}`);
        }}
        ondragenter={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, `stack-${stack.id}`);
        }}
        ondragleave={(e) => handleDragLeave(e)}
        ondrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDrop(e, 'stack', stack.id);
        }}
        onclick={(e) => {
          // Only toggle if clicking the header itself, not action buttons
          if (!isDragging && !(e.target as HTMLElement).closest('.stack-actions')) {
            toggleStackCollapse(stack.id);
          }
        }}
        onkeydown={(e) => {
          if ((e.key === 'Enter' || e.key === ' ') && !isDragging && !(e.target as HTMLElement).closest('.stack-actions')) {
            e.preventDefault();
            toggleStackCollapse(stack.id);
          }
        }}
      >
        <div class="stack-toggle">
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor" class:rotated={!isCollapsed}>
            <path d="M8 5v14l11-7z"/>
          </svg>
        </div>
        <div class="stack-info">
          <span class="stack-name">{stack.name}</span>
          <span class="stack-meta">{stackInstances.length} services · {runningCount} running</span>
          {#if stack.description}
            <span class="stack-description">{stack.description}</span>
          {/if}
        </div>
        <div class="stack-actions" role="group" aria-label="Stack actions" onclick={(e) => e.stopPropagation()}>
          {#if onStartStack && !isStackFullyRunning(stack.id)}
            <button class="icon-btn success" onclick={() => onStartStack?.(stack.id)} title="Start All">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <polygon points="6,4 20,12 6,20"/>
              </svg>
            </button>
          {/if}
          {#if onStopStack && runningCount > 0}
            <button class="icon-btn danger" onclick={() => onStopStack?.(stack.id)} title="Stop All">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <rect x="6" y="6" width="12" height="12" rx="1"/>
              </svg>
            </button>
          {/if}
          {#if onExportStack}
            <button class="icon-btn" onclick={() => onExportStack?.(stack.id)} title="Export Stack">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                <polyline points="17 8 12 3 7 8"/>
                <line x1="12" y1="3" x2="12" y2="15"/>
              </svg>
            </button>
          {/if}
          {#if onDeleteStack}
            <button class="icon-btn danger" onclick={() => onDeleteStack?.(stack.id)} title="Delete Stack">
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="3 6 5 6 21 6"/>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
              </svg>
            </button>
          {/if}
        </div>
      </div>
      {#if !isCollapsed}
        <div class="instances-grid stack-instances">
          <div class="grid-body">
            {#each stackInstances as instance, index (instance.id)}
              {@render instanceRow(instance, false, index)}
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/each}
</div>

{#snippet instanceRow(instance: Instance, showCheckbox: boolean = false, index: number = 0)}
            <div
              class="grid-row"
              class:dragging={draggedInstanceId === instance.id}
              class:drag-over={dragOverInstanceId === instance.id && draggedInstanceId !== instance.id}
              data-index={index}
              data-instance-id={instance.id}
            >
              {#if showCheckbox && selectionMode}
                <div class="grid-cell checkbox-col">
                  <input
                    type="checkbox"
                    checked={selectedInstances.has(instance.id)}
                    onchange={() => toggleInstanceSelection(instance.id)}
                  />
                </div>
              {/if}
              {#if !selectionMode}
                <div class="grid-cell drag-col">
                  <div
                    class="drag-handle"
                    role="button"
                    tabindex="0"
                    onmousedown={(e) => handleMouseDragStart(e, instance.id)}
                    title="Drag to reorder instance"
                    aria-label="Drag to reorder instance"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                      <circle cx="9" cy="5" r="1.5"/>
                      <circle cx="9" cy="12" r="1.5"/>
                      <circle cx="9" cy="19" r="1.5"/>
                      <circle cx="15" cy="5" r="1.5"/>
                      <circle cx="15" cy="12" r="1.5"/>
                      <circle cx="15" cy="19" r="1.5"/>
                    </svg>
                  </div>
                </div>
              {/if}
              <div class="grid-cell status-cell">
                {#if instance.running}
                  {#if instance.healthy}
                    <span class="status-dot running" title="Running"></span>
                  {:else if instance.healthy === false}
                    <span class="status-dot unhealthy" title="Unhealthy"></span>
                  {:else}
                    <span class="status-dot starting" title="Starting"></span>
                  {/if}
                {:else}
                  <span class="status-dot stopped" title="Stopped"></span>
                {/if}
              </div>
              <div class="grid-cell name">
                <div class="service-icon" style="--brand-color: {getServiceStyle(instance.service_type).color}" title="{instance.service_type}">
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                    {@html getServiceStyle(instance.service_type).icon}
                  </svg>
                </div>
                <span class="instance-name">{instance.name}</span>
              </div>
              <div class="grid-cell domain">
                {#if instance.mapped_domains && instance.mapped_domains.length > 0}
                  <div class="mapped-domains">
                    {#each instance.mapped_domains as domain}
                      <div class="url-cell">
                        {#if instance.running && resolverInstalled}
                          <a
                            href="https://{domain}"
                            target="_blank"
                            rel="noopener noreferrer"
                            class="url-link"
                            title={domain}
                          >
                            {domain}
                          </a>
                        {:else}
                          <span class="domain-inactive">{domain}</span>
                        {/if}
                        <button
                          class="copy-btn"
                          onclick={() => copyToClipboard(domain, `${instance.id}-${domain}`)}
                          title="Copy domain"
                        >
                          {copiedId === `${instance.id}-${domain}` ? "✓" : "⧉"}
                        </button>
                        <button
                          class="delete-domain-btn icon-btn danger"
                          onclick={() => handleDeleteDomain(domain, instance.id)}
                          title="Remove this domain"
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <line x1="18" y1="6" x2="6" y2="18"></line>
                            <line x1="6" y1="6" x2="18" y2="18"></line>
                          </svg>
                        </button>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <span class="domain-disabled">—</span>
                {/if}
              </div>
              <div class="grid-cell port" title="Port">{instance.port}</div>
              <div class="grid-cell pid" title="Process ID">{instance.pid ?? "-"}</div>
              <div class="grid-cell actions">
                {#if instance.running}
                  <button
                    class="icon-btn danger"
                    onclick={() => onStop(instance.id)}
                    disabled={actionLoading[instance.id]}
                    title="Stop"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none">
                      <rect x="6" y="6" width="12" height="12" rx="1"></rect>
                    </svg>
                  </button>
                  {#if isPm2Managed(instance)}
                    <button
                      class="icon-btn"
                      onclick={() => onRestart(instance.id)}
                      disabled={actionLoading[instance.id]}
                      title="Restart"
                    >
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M21 2v6h-6"></path>
                        <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
                        <path d="M3 22v-6h6"></path>
                        <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
                      </svg>
                    </button>
                  {/if}
                {:else if needsInitialization(instance)}
                  <button
                    class="icon-btn warning"
                    onclick={() => onInitialize(instance.id)}
                    disabled={initializingInstances[instance.id]}
                    title="Initialize (npm install)"
                  >
                    {#if initializingInstances[instance.id]}
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="spinning">
                        <circle cx="12" cy="12" r="10" stroke-dasharray="32" stroke-dashoffset="8"></circle>
                      </svg>
                    {:else}
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                        <polyline points="7 10 12 15 17 10"></polyline>
                        <line x1="12" y1="15" x2="12" y2="3"></line>
                      </svg>
                    {/if}
                  </button>
                {:else}
                  {@const disabledReason = getStartDisabledReason(instance)}
                  <button
                    class="icon-btn {disabledReason && !actionLoading[instance.id] ? 'warning' : 'success'}"
                    onclick={() => onStart(instance.id)}
                    disabled={isStartDisabled(instance)}
                    title={disabledReason || "Start"}
                  >
                    {#if disabledReason && !actionLoading[instance.id]}
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <circle cx="12" cy="12" r="10"></circle>
                        <line x1="12" y1="8" x2="12" y2="12"></line>
                        <line x1="12" y1="16" x2="12.01" y2="16"></line>
                      </svg>
                    {:else}
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="currentColor" stroke="none">
                        <polygon points="6,4 20,12 6,20"></polygon>
                      </svg>
                    {/if}
                  </button>
                {/if}
                <button
                  class="icon-btn"
                  onclick={() => onOpenSettings(instance)}
                  title="Settings"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="3"></circle>
                    <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
                  </svg>
                </button>
                <button
                  class="icon-btn"
                  onclick={() => onViewLogs(instance.id, instance.name)}
                  title="View Logs"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                    <polyline points="14 2 14 8 20 8"></polyline>
                    <line x1="16" y1="13" x2="8" y2="13"></line>
                    <line x1="16" y1="17" x2="8" y2="17"></line>
                  </svg>
                </button>
                <button
                  class="icon-btn"
                  onclick={() => onViewEnv(instance.id, instance.name, instance.service_type)}
                  title="View ENV Variables"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="16 18 22 12 16 6"></polyline>
                    <polyline points="8 6 2 12 8 18"></polyline>
                  </svg>
                </button>
                <button
                  class="icon-btn"
                  onclick={() => onViewInfo(instance.id, instance.name, instance.service_type)}
                  title="View Information"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <circle cx="12" cy="12" r="10"></circle>
                    <line x1="12" y1="16" x2="12" y2="12"></line>
                    <line x1="12" y1="8" x2="12.01" y2="8"></line>
                  </svg>
                </button>
                <button
                  class="icon-btn danger"
                  onclick={() => onDelete(instance.id, instance.name)}
                  disabled={actionLoading[instance.id]}
                  title="Delete"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="3 6 5 6 21 6"></polyline>
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                  </svg>
                </button>
              </div>
            </div>
{/snippet}

<style>
  .instances-section {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .section-header h2 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .refresh-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    border-radius: 6px;
    cursor: pointer;
    color: #86868b;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    .refresh-btn {
      color: #98989d;
    }

    .refresh-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
  }

  .card {
    background: white;
    border-radius: 12px;
    padding: 1rem;
    border: 1px solid #e5e5e5;
    overflow-x: auto;
  }

  .create-form {
    background: white;
    border-radius: 12px;
    padding: 1.5rem;
    border: 1px solid #e5e5e5;
  }

  .form-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  label span {
    font-size: 0.875rem;
    font-weight: 500;
  }

  input, select {
    padding: 0.5rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
    color: inherit;
  }

  @media (prefers-color-scheme: dark) {
    input, select {
      background: #1c1c1e;
      border-color: #3a3a3c;
    }
  }

  input:focus, select:focus {
    outline: none;
    border-color: #007aff;
  }

  .form-hint {
    margin: 0.5rem 0 0;
    font-size: 0.875rem;
    color: #ff9500;
  }

  .domain-input-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .domain-input {
    flex: 1;
  }

  .domain-suffix {
    color: #86868b;
    font-size: 0.875rem;
    font-family: monospace;
  }

  .optional-label {
    color: #86868b;
    font-size: 0.75rem;
    font-weight: 400;
  }

  .field-hint {
    display: block;
    margin-top: 0.25rem;
    font-size: 0.75rem;
    color: #86868b;
  }

  .instances-grid {
    width: 100%;
    font-size: 0.875rem;
    --grid-columns: 32px auto 1fr 2fr auto auto auto;
  }

  .grid-body {
    display: contents;
  }

  .grid-row {
    display: grid;
    grid-template-columns: var(--grid-columns);
    gap: 0;
    transition: transform 0.15s ease, opacity 0.15s ease, box-shadow 0.15s ease;
  }

  .drag-handle {
    cursor: grab;
    user-select: none;
  }

  .drag-handle:active {
    cursor: grabbing;
  }

  :global(body.dragging-instance) {
    cursor: grabbing !important;
    user-select: none;
  }

  .grid-cell {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid #f0f0f0;
    display: flex;
    align-items: center;
    min-height: 36px;
    overflow: hidden;
  }

  .grid-row:last-child .grid-cell {
    border-bottom: none;
  }

  .grid-row:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .grid-cell {
      border-bottom-color: #38383a;
    }
    .grid-row:hover {
      background: rgba(255, 255, 255, 0.05);
    }
  }

  .grid-cell.name {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .grid-cell.name .service-icon {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--brand-color) 15%, transparent);
    color: var(--brand-color);
    flex-shrink: 0;
  }

  .grid-cell.name .service-icon svg {
    width: 14px;
    height: 14px;
  }

  .grid-cell.name .instance-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .grid-cell.domain {
    min-width: 0;
    overflow: hidden;
  }

  .grid-cell.port {
    font-family: monospace;
    white-space: nowrap;
    justify-content: flex-start;
  }

  .grid-cell.pid {
    font-family: monospace;
    color: #86868b;
    white-space: nowrap;
    justify-content: flex-start;
  }

  .url-cell {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .url-link {
    color: #007aff;
    text-decoration: none;
    font-size: 0.8125rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .url-link:hover {
    text-decoration: underline;
  }

  .copy-btn {
    background: none;
    border: none;
    padding: 0.125rem 0.25rem;
    cursor: pointer;
    color: #86868b;
    font-size: 0.75rem;
    border-radius: 3px;
    transition: all 0.15s ease;
  }

  .copy-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    .copy-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
  }

  .delete-domain-btn {
    padding: 0.25rem;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.15s ease;
    background: none;
    border: none;
    cursor: pointer;
    border-radius: 3px;
  }

  .url-cell:hover .delete-domain-btn {
    opacity: 1;
  }

  .delete-domain-btn:hover {
    background: rgba(239, 68, 68, 0.1);
  }

  .delete-domain-btn svg {
    color: #ef4444;
  }

  .domain-inactive, .domain-disabled {
    color: #86868b;
    font-size: 0.8125rem;
  }

  .grid-cell.actions {
    white-space: nowrap;
    display: flex;
    gap: 0.25rem;
    flex-wrap: nowrap;
    justify-content: flex-start;
  }

  .status-cell {
    justify-content: center;
  }

  .status-dot {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    cursor: default;
  }

  .status-dot.running { background: #22c55e; box-shadow: 0 0 6px rgba(34, 197, 94, 0.5); }
  .status-dot.stopped { background: #9ca3af; }
  .status-dot.starting { background: #f59e0b; animation: pulse 1.5s ease-in-out infinite; }
  .status-dot.unhealthy { background: #ef4444; animation: pulse 1s ease-in-out infinite; }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn.secondary {
    background: #e5e5e5;
    color: #1d1d1f;
  }

  .btn.secondary:hover:not(:disabled) {
    background: #d1d1d6;
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  @media (prefers-color-scheme: dark) {
    .btn.secondary {
      background: #3a3a3c;
      color: #f5f5f7;
    }

    .btn.secondary:hover:not(:disabled) {
      background: #48484a;
    }
  }

  /* Icon button styles */
  .icon-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    cursor: pointer;
    color: #86868b;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1);
    color: #dc2626;
  }

  .icon-btn.success {
    color: #22c55e;
  }

  .icon-btn.success:hover {
    background: rgba(34, 197, 94, 0.1);
    color: #16a34a;
  }

  .icon-btn.warning {
    color: #f59e0b;
  }

  .icon-btn.warning:hover {
    background: rgba(245, 158, 11, 0.1);
    color: #d97706;
  }

  .icon-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @media (prefers-color-scheme: dark) {
    .icon-btn {
      color: #98989d;
    }
    .icon-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
    .icon-btn.danger:hover {
      background: rgba(239, 68, 68, 0.2);
      color: #ef4444;
    }
    .icon-btn.success {
      color: #4ade80;
    }
    .icon-btn.success:hover {
      background: rgba(74, 222, 128, 0.2);
      color: #86efac;
    }
    .icon-btn.warning {
      color: #fbbf24;
    }
    .icon-btn.warning:hover {
      background: rgba(251, 191, 36, 0.2);
      color: #fcd34d;
    }
  }

  .loading, .empty {
    text-align: center;
    color: #86868b;
    padding: 2rem;
  }

  /* Light mode explicit overrides */
  :global(:root[data-theme="light"]) .card {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .create-form {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .grid-cell {
    border-bottom-color: #f0f0f0 !important;
  }

  :global(:root[data-theme="light"]) .grid-row:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  :global(:root[data-theme="light"]) .status-dot.running { background: #22c55e !important; }
  :global(:root[data-theme="light"]) .status-dot.stopped { background: #9ca3af !important; }
  :global(:root[data-theme="light"]) .status-dot.starting { background: #f59e0b !important; }
  :global(:root[data-theme="light"]) .status-dot.unhealthy { background: #ef4444 !important; }

  :global(:root[data-theme="light"]) .refresh-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .icon-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1) !important;
    color: #dc2626 !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.success {
    color: #22c55e !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.success:hover {
    background: rgba(34, 197, 94, 0.1) !important;
    color: #16a34a !important;
  }

  :global(:root[data-theme="light"]) .copy-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .copy-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }

  /* Dark mode explicit overrides */
  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .create-form {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .grid-cell {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .grid-row:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="dark"]) .status-dot.running { background: #22c55e !important; }
  :global(:root[data-theme="dark"]) .status-dot.stopped { background: #6b7280 !important; }
  :global(:root[data-theme="dark"]) .status-dot.starting { background: #f59e0b !important; }
  :global(:root[data-theme="dark"]) .status-dot.unhealthy { background: #ef4444 !important; }

  :global(:root[data-theme="dark"]) .refresh-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2) !important;
    color: #ef4444 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.success {
    color: #4ade80 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.success:hover {
    background: rgba(74, 222, 128, 0.2) !important;
    color: #86efac !important;
  }

  :global(:root[data-theme="dark"]) .copy-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .copy-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) input,
  :global(:root[data-theme="dark"]) select {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
  }

  /* Header actions */
  .header-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  /* Stack group styles */
  .stack-group {
    margin-bottom: 1rem;
    border: 1px solid #e5e5e5;
    border-radius: 8px;
    overflow: hidden;
    background: white;
  }

  .stack-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: #f5f5f7;
    cursor: pointer;
    user-select: none;
    position: relative;
  }

  .stack-header:hover {
    background: #ebebed;
  }

  /* Allow header to receive drag events while children handle clicks */
  .stack-header .stack-toggle,
  .stack-header .stack-info {
    pointer-events: none;
  }

  .stack-header .stack-actions {
    pointer-events: auto;
  }

  .stack-header .stack-actions button {
    pointer-events: auto;
  }

  .stack-toggle {
    color: #86868b;
    transition: transform 0.2s ease;
    display: flex;
    align-items: center;
  }

  .stack-toggle svg.rotated {
    transform: rotate(90deg);
  }

  .stack-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .stack-name {
    font-weight: 600;
    font-size: 0.9375rem;
  }

  .stack-meta {
    font-size: 0.75rem;
    color: #86868b;
  }

  .stack-description {
    font-size: 0.8125rem;
    color: #86868b;
    margin-top: 0.125rem;
  }

  .stack-actions {
    display: flex;
    gap: 0.25rem;
  }

  .stack-instances {
    border: none;
  }

  /* Selection bar */
  .selection-bar {
    position: sticky;
    bottom: 1rem;
    left: 0;
    right: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: #1d1d1f;
    color: white;
    border-radius: 10px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 100;
    margin-top: 1rem;
  }

  .selection-count {
    font-size: 0.875rem;
    font-weight: 500;
  }

  .selection-actions {
    display: flex;
    gap: 0.5rem;
  }

  .selection-bar .btn.secondary {
    background: rgba(255, 255, 255, 0.15);
    color: white;
  }

  .selection-bar .btn.secondary:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  /* Checkbox column */
  .checkbox-col {
    width: 32px;
    justify-content: center;
  }

  .checkbox-col input[type="checkbox"] {
    width: 16px;
    height: 16px;
    cursor: pointer;
    accent-color: #007aff;
  }

  /* Modal styles */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: white;
    border-radius: 12px;
    width: 90%;
    max-width: 480px;
    max-height: 90vh;
    overflow-y: auto;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid #e5e5e5;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.5rem;
    color: #86868b;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  .close-btn:hover {
    color: #1d1d1f;
  }

  .modal-body {
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .modal-body textarea {
    min-height: 80px;
    resize: vertical;
    padding: 0.5rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    font-family: inherit;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.25rem;
    border-top: 1px solid #e5e5e5;
  }

  .selected-instances-preview {
    background: #f5f5f7;
    border-radius: 8px;
    padding: 0.75rem 1rem;
  }

  .selected-instances-preview span {
    font-size: 0.8125rem;
    color: #86868b;
    font-weight: 500;
  }

  .selected-instances-preview ul {
    margin: 0.5rem 0 0 0;
    padding-left: 1.25rem;
  }

  .selected-instances-preview li {
    font-size: 0.875rem;
    color: #1d1d1f;
  }

  :global(:root[data-theme="dark"]) .stack-group {
    border-color: #38383a !important;
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .stack-header {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .stack-header:hover {
    background: #3a3a3c !important;
  }

  :global(:root[data-theme="dark"]) .modal {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .modal-header {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .close-btn:hover {
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .modal-footer {
    border-top-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .modal-body textarea {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .selected-instances-preview {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .selected-instances-preview li {
    color: #f5f5f7 !important;
  }

  /* Mapped domains display */
  .mapped-domains {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    width: 100%;
    overflow: hidden;
  }

  .mapped-domains .url-cell {
    font-size: 0.8125rem;
  }

  /* Drag and Drop Styles */
  .drag-col {
    width: 32px;
    justify-content: center;
    padding: 0.5rem 0.25rem !important;
  }

  .drag-handle {
    padding: 0.25rem;
    cursor: grab;
    color: #86868b;
    opacity: 0.3;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.15s ease;
    user-select: none;
  }

  .drag-handle:hover {
    opacity: 0.6;
    background: rgba(0, 0, 0, 0.05);
  }

  .drag-handle:active {
    cursor: grabbing;
  }

  .dragging .drag-handle {
    cursor: grabbing !important;
    opacity: 1;  /* Full opacity to show it's the active handle */
  }

  @media (prefers-color-scheme: dark) {
    .drag-handle:hover {
      background: rgba(255, 255, 255, 0.1);
    }
  }

  .drop-target-active {
    background: rgba(59, 130, 246, 0.1) !important;
    border: 2px dashed #3b82f6 !important;
    outline: 2px solid #3b82f6;
    outline-offset: -2px;
    transition: all 0.2s ease;
  }

  .standalone-drop-zone {
    border: 2px dashed #94a3b8;
    border-radius: 8px;
    padding: 24px;
    margin: 0 0 16px 0;
    text-align: center;
    color: #64748b;
    background: rgba(148, 163, 184, 0.05);
    transition: all 0.2s ease;
  }

  .standalone-drop-zone.drop-target-active {
    background: rgba(16, 185, 129, 0.1);
    border-color: #10b981;
    color: #10b981;
  }

  .drop-zone-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    font-size: 14px;
    font-weight: 500;
  }

  .grid-row.dragging {
    opacity: 0.6;  /* Slightly less transparent to remain visible */
    transform: scale(0.98);  /* Slight shrink effect */
    background: rgba(59, 130, 246, 0.08) !important;  /* Light blue tint */
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);  /* Elevation shadow */
    border: 2px dashed #3b82f6 !important;  /* Blue dashed border */
    cursor: grabbing !important;  /* Maintain grabbing cursor */
  }

  @media (prefers-color-scheme: dark) {
    .grid-row.dragging {
      opacity: 0.7;  /* Better visibility in dark mode */
      background: rgba(59, 130, 246, 0.12) !important;
      box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);  /* Stronger shadow for dark */
    }
  }

  .grid-row.drag-over {
    border-top: 3px solid #3b82f6;
    border-bottom: 3px solid #3b82f6;
  }

  /* DnD Error notification */
  .dnd-error {
    background: #ff3b30;
    color: white;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    animation: slideIn 0.2s ease;
  }

  @keyframes slideIn {
    from {
      transform: translateY(-10px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .dnd-error .close-btn {
    background: none;
    border: none;
    color: white;
    font-size: 1.25rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  @media (prefers-color-scheme: dark) {
    .dnd-error {
      background: #dc2626;
    }
  }

  /* Drag-and-drop uses mouse events, no special pointer-events needed */
</style>
