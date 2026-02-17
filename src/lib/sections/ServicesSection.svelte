<script lang="ts">
  interface ServiceInfo {
    id: string;
    display_name: string;
    default_port: number;
    config_fields: { key: string; label: string; field_type: string; required: boolean; default: string | null }[];
    available: boolean;
    is_homebrew: boolean;
  }

  interface VersionInfo {
    version: string;
    is_latest: boolean;
    label?: string;
  }

  interface DownloadProgress {
    service_type: string;
    downloaded: number;
    total: number;
    percentage: number;
    phase: string;
  }

  let {
    serviceTypes = [],
    installedVersions = {},
    downloading = {},
    downloadProgress = {},
    availableVersions = {},
    selectedVersions = {},
    loadingVersions = {},
    showVersionSelector = {},
    // Event handlers
    onFetchVersions,
    onDownload,
    onCancelVersionSelector,
    onDeleteVersion,
  }: {
    serviceTypes: ServiceInfo[];
    installedVersions: Record<string, string[]>;
    downloading: Record<string, boolean>;
    downloadProgress: Record<string, DownloadProgress>;
    availableVersions: Record<string, VersionInfo[]>;
    selectedVersions: Record<string, string>;
    loadingVersions: Record<string, boolean>;
    showVersionSelector: Record<string, boolean>;
    onFetchVersions: (serviceId: string) => void;
    onDownload: (serviceId: string, version: string) => void;
    onCancelVersionSelector: (serviceId: string) => void;
    onDeleteVersion: (serviceId: string, version: string) => void;
  } = $props();

  // Service brand colors and icons (extracted from simple-icons)
  const serviceStyles: Record<string, { color: string; icon: string }> = {
    meilisearch: {
      color: '#FF5CAA',
      icon: `<path d="m6.505 18.998 4.434-11.345a4.168 4.168 0 0 1 3.882-2.651h2.674l-4.434 11.345a4.169 4.169 0 0 1-3.883 2.651H6.505Zm6.505 0 4.434-11.345a4.169 4.169 0 0 1 3.883-2.651H24l-4.434 11.345a4.168 4.168 0 0 1-3.882 2.651H13.01Zm-13.01 0L4.434 7.653a4.168 4.168 0 0 1 3.882-2.651h2.674L6.556 16.347a4.169 4.169 0 0 1-3.883 2.651H0Z"/>`
    },
    typesense: {
      color: '#D52E63',
      icon: `<path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/>`
    },
    mongodb: {
      color: '#47A248',
      icon: `<path d="M17.193 9.555c-1.264-5.58-4.252-7.414-4.573-8.115-.28-.394-.53-.954-.735-1.44-.036.495-.055.685-.523 1.184-.723.566-4.438 3.682-4.74 10.02-.282 5.912 4.27 9.435 4.888 9.884l.07.05A73.49 73.49 0 0111.91 24h.481c.114-1.032.284-2.056.51-3.07.417-.296.604-.463.85-.693a11.342 11.342 0 003.639-8.464c.01-.814-.103-1.662-.197-2.218zm-5.336 8.195s0-8.291.275-8.29c.213 0 .49 10.695.49 10.695-.381-.045-.765-1.76-.765-2.405z"/>`
    },
    minio: {
      color: '#C72E49',
      icon: `<path d="M13.2072.006c-.6216-.0478-1.2.1943-1.6211.582a2.15 2.15 0 0 0-.0938 3.0352l3.4082 3.5507a3.042 3.042 0 0 1-.664 4.6875l-.463.2383V7.2853a15.4198 15.4198 0 0 0-8.0174 10.4862v.0176l6.5487-3.3281v7.621L13.7794 24V13.6817l.8965-.4629a4.4432 4.4432 0 0 0 1.2207-7.0292l-3.371-3.5254a.7489.7489 0 0 1 .037-1.0547.7522.7522 0 0 1 1.0567.0371l.4668.4863-.006.0059 4.0704 4.2441a.0566.0566 0 0 0 .082 0 .06.06 0 0 0 0-.0703l-3.1406-5.1425-.1484.1425.1484-.1445C14.4945.3926 13.8287.0538 13.2072.006Zm-.9024 9.8652v2.9941l-4.1523 2.1484a13.9787 13.9787 0 0 1 2.7676-3.9277 14.1784 14.1784 0 0 1 1.3847-1.2148z"/>`
    },
    frankenphp: {
      color: '#777BB4',
      icon: `<path d="M7.01 10.207h-.944l-.515 2.648h.838c.556 0 .97-.105 1.242-.314.272-.21.455-.559.55-1.049.092-.47.05-.802-.124-.995-.175-.193-.523-.29-1.047-.29zM12 5.688C5.373 5.688 0 8.514 0 12s5.373 6.313 12 6.313S24 15.486 24 12c0-3.486-5.373-6.312-12-6.312zm-3.26 7.451c-.261.25-.575.438-.917.551-.336.108-.765.164-1.285.164H5.357l-.327 1.681H3.652l1.23-6.326h2.65c.797 0 1.378.209 1.744.628.366.418.476 1.002.33 1.752a2.836 2.836 0 0 1-.305.847c-.143.255-.33.49-.561.703zm4.024.715l.543-2.799c.063-.318.039-.536-.068-.651-.107-.116-.336-.174-.687-.174H11.46l-.704 3.625H9.388l1.23-6.327h1.367l-.327 1.682h1.218c.767 0 1.295.134 1.586.401s.378.7.263 1.299l-.572 2.944h-1.389zm7.597-2.265a2.782 2.782 0 0 1-.305.847c-.143.255-.33.49-.561.703a2.44 2.44 0 0 1-.917.551c-.336.108-.765.164-1.286.164h-1.18l-.327 1.682h-1.378l1.23-6.326h2.649c.797 0 1.378.209 1.744.628.366.417.477 1.001.331 1.751zM17.766 10.207h-.943l-.516 2.648h.838c.557 0 .971-.105 1.242-.314.272-.21.455-.559.551-1.049.092-.47.049-.802-.125-.995s-.524-.29-1.047-.29z"/>`
    },
    mariadb: {
      color: '#003545',
      icon: `<path d="M23.157 4.412c-.676.284-.79.31-1.673.372-.65.045-.757.057-1.212.209-.75.246-1.395.75-2.02 1.59-.296.398-1.249 1.913-1.249 1.988 0 .057-.65.998-.915 1.32-.574.713-1.08 1.079-2.14 1.59-.77.36-1.224.524-4.102 1.477-1.073.353-2.133.738-2.367.864-.852.449-1.515 1.036-2.203 1.938-1.003 1.32-.972 1.313-3.042.947a12.264 12.264 0 00-.675-.063c-.644-.05-1.023.044-1.332.334L0 17.193l.177.088c.094.05.353.234.561.398.215.17.461.347.55.391.088.044.17.088.183.101.012.013-.089.17-.228.353-.435.581-.593.871-.574 1.048.019.164.032.17.43.17.517-.006.826-.056 1.261-.208.65-.233 2.058-.94 2.784-1.4.776-.5 1.717-.998 1.956-1.042.082-.02.354-.07.594-.114.58-.107 1.464-.095 2.587.05.108.013.373.045.6.064.227.025.43.057.454.076.026.012.474.037.998.056.934.026 1.104.007 1.3-.189.126-.133.385-.631.498-.985.209-.643.417-.921.366-.492-.113.966-.322 1.692-.713 2.411-.259.499-.663 1.092-.934 1.395-.322.347-.315.36.088.315.619-.063 1.471-.397 2.096-.82.827-.562 1.647-1.691 2.19-3.03.107-.27.22-.22.183.083-.013.094-.038.315-.057.498l-.031.328.353-.202c.833-.48 1.414-1.262 2.127-2.884.227-.518.877-2.922 1.073-3.976a9.64 9.64 0 01.271-1.042c.127-.429.196-.555.48-.858.183-.19.625-.555.978-.808.72-.505.953-.75 1.187-1.205.208-.417.284-1.13.132-1.357-.132-.202-.284-.196-.763.006Z"/>`
    },
    postgresql: {
      color: '#4169E1',
      icon: `<path d="M23.5594 14.7228a.5269.5269 0 0 0-.0563-.1191c-.139-.2632-.4768-.3418-1.0074-.2321-1.6533.3411-2.2935.1312-2.5256-.0191 1.342-2.0482 2.445-4.522 3.0411-6.8297.2714-1.0507.7982-3.5237.1222-4.7316a1.5641 1.5641 0 0 0-.1509-.235C21.6931.9086 19.8007.0248 17.5099.0005c-1.4947-.0158-2.7705.3461-3.1161.4794a9.449 9.449 0 0 0-.5159-.0816 8.044 8.044 0 0 0-1.3114-.1278c-1.1822-.0184-2.2038.2642-3.0498.8406-.8573-.3211-4.7888-1.645-7.2219.0788C.9359 2.1526.3086 3.8733.4302 6.3043c.0409.818.5069 3.334 1.2423 5.7436.4598 1.5065.9387 2.7019 1.4334 3.582.553.9942 1.1259 1.5933 1.7143 1.7895.4474.1491 1.1327.1441 1.8581-.7279.8012-.9635 1.5903-1.8258 1.9446-2.2069.4351.2355.9064.3625 1.39.3772a.0569.0569 0 0 0 .0004.0041 11.0312 11.0312 0 0 0-.2472.3054c-.3389.4302-.4094.5197-1.5002.7443-.3102.064-1.1344.2339-1.1464.8115-.0025.1224.0329.2309.0919.3268.2269.4231.9216.6097 1.015.6331 1.3345.3335 2.5044.092 3.3714-.6787-.017 2.231.0775 4.4174.3454 5.0874.2212.5529.7618 1.9045 2.4692 1.9043.2505 0 .5263-.0291.8296-.0941 1.7819-.3821 2.5557-1.1696 2.855-2.9059.1503-.8707.4016-2.8753.5388-4.1012.0169-.0703.0357-.1207.057-.1362.0007-.0005.0697-.0471.4272.0307a.3673.3673 0 0 0 .0443.0068l.2539.0223.0149.001c.8468.0384 1.9114-.1426 2.5312-.4308.6438-.2988 1.8057-1.0323 1.5951-1.6698z"/>`
    },
    redis: {
      color: '#FF4438',
      icon: `<path d="M22.71 13.145c-1.66 2.092-3.452 4.483-7.038 4.483-3.203 0-4.397-2.825-4.48-5.12.701 1.484 2.073 2.685 4.214 2.63 4.117-.133 6.94-3.852 6.94-7.239 0-4.05-3.022-6.972-8.268-6.972-3.752 0-8.4 1.428-11.455 3.685C2.59 6.937 3.885 9.958 4.35 9.626c2.648-1.904 4.748-3.13 6.784-3.744C8.12 9.244.886 17.05 0 18.425c.1 1.261 1.66 4.648 2.424 4.648.232 0 .431-.133.664-.365a100.49 100.49 0 0 0 5.54-6.765c.222 3.104 1.748 6.898 6.014 6.898 3.819 0 7.604-2.756 9.33-8.965.2-.764-.73-1.361-1.261-.73zm-4.349-5.013c0 1.959-1.926 2.922-3.685 2.922-.941 0-1.664-.247-2.235-.568 1.051-1.592 2.092-3.225 3.21-4.973 1.972.334 2.71 1.43 2.71 2.619z"/>`
    },
    valkey: {
      color: '#00A89D',
      icon: `<path d="M12 2L3 7v10l9 5 9-5V7l-9-5zm0 2.18l6.37 3.53L12 11.24 5.63 7.71 12 4.18zM5 9.23l6 3.33v6.22l-6-3.33V9.23zm8 9.55v-6.22l6-3.33v6.22l-6 3.33z"/>`
    },
    mailpit: {
      color: '#0891b2',
      icon: `<path d="M20 4H4c-1.1 0-1.99.9-1.99 2L2 18c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 4l-8 5-8-5V6l8 5 8-5v2z"/>`
    },
    beanstalkd: {
      color: '#22c55e',
      icon: `<path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>`
    },
    memcached: {
      color: '#059669',
      icon: `<path d="M2 20h20v-4H2v4zm2-3h2v2H4v-2zM2 4v4h20V4H2zm4 3H4V5h2v2zm-4 7h20v-4H2v4zm2-3h2v2H4v-2z"/>`
    },
    frpc: {
      color: '#00ADD8',
      icon: `<path d="M1.811 10.231c-.047 0-.058-.023-.035-.059l.246-.315c.023-.035.081-.058.128-.058h4.172c.046 0 .058.035.035.07l-.199.303c-.023.036-.082.07-.117.07zM.047 11.306c-.047 0-.059-.023-.035-.058l.245-.316c.023-.035.082-.058.129-.058h5.328c.047 0 .07.035.058.07l-.093.28c-.012.047-.058.07-.105.07zm2.828 1.075c-.047 0-.059-.035-.035-.07l.163-.292c.023-.035.07-.07.117-.07h2.337c.047 0 .07.035.07.082l-.023.28c0 .047-.047.082-.082.082zm12.129-2.36c-.736.187-1.239.327-1.963.514-.176.046-.187.058-.34-.117-.174-.199-.303-.327-.548-.444-.737-.362-1.45-.257-2.115.175-.795.514-1.204 1.274-1.192 2.22.011.935.654 1.706 1.577 1.835.795.105 1.46-.175 1.987-.77.105-.13.198-.27.315-.434H10.47c-.245 0-.304-.152-.222-.35.152-.362.432-.97.596-1.274a.315.315 0 01.292-.187h4.253c-.023.316-.023.631-.07.947a4.983 4.983 0 01-.958 2.29c-.841 1.11-1.94 1.8-3.33 1.986-1.145.152-2.209-.07-3.143-.77-.865-.655-1.356-1.52-1.484-2.595-.152-1.274.222-2.419.993-3.424.83-1.086 1.928-1.776 3.272-2.02 1.098-.2 2.15-.07 3.096.571.62.41 1.063.97 1.356 1.648.07.105.023.164-.117.2m3.868 6.461c-1.064-.024-2.034-.328-2.852-1.029a3.665 3.665 0 01-1.262-2.255c-.21-1.32.152-2.489.947-3.529.853-1.122 1.881-1.706 3.272-1.95 1.192-.21 2.314-.095 3.33.595.923.63 1.496 1.484 1.648 2.605.198 1.578-.257 2.863-1.344 3.962-.771.783-1.718 1.273-2.805 1.495-.315.06-.63.07-.934.106zm2.78-4.72c-.011-.153-.011-.27-.034-.387-.21-1.157-1.274-1.81-2.384-1.554-1.087.245-1.788.935-2.045 2.033-.21.912.234 1.835 1.075 2.21.643.28 1.285.244 1.905-.07.923-.48 1.425-1.228 1.484-2.233z"/>`
    },
    nodered: {
      color: '#8F0000',
      icon: `<path d="M3 0C1.338 0 0 1.338 0 3v6.107h2.858c1.092 0 1.97.868 1.964 1.96v.021c.812-.095 1.312-.352 1.674-.683.416-.382.69-.91 1.016-1.499.325-.59.71-1.244 1.408-1.723.575-.395 1.355-.644 2.384-.686v-.45c0-1.092.88-1.976 1.972-1.976h7.893c1.091 0 1.974.884 1.974 1.976v1.942c0 1.091-.883 2.029-1.974 2.029h-7.893c-1.092 0-1.972-.938-1.972-2.03v-.453c-.853.037-1.408.236-1.798.504-.48.33-.774.802-1.086 1.368-.312.565-.63 1.22-1.222 1.763l-.077.069c3.071.415 4.465 1.555 5.651 2.593 1.39 1.215 2.476 2.275 6.3 2.288v-.46c0-1.092.894-1.946 1.986-1.946H24V3c0-1.662-1.338-3-3-3zm10.276 5.41c-.369 0-.687.268-.687.637v1.942c0 .368.318.636.687.636h7.892a.614.614 0 0 0 .635-.636V6.047a.614.614 0 0 0-.635-.636zM0 10.448v3.267h2.858a.696.696 0 0 0 .678-.69v-1.942c0-.368-.31-.635-.678-.635zm4.821 1.67v.907A1.965 1.965 0 0 1 2.858 15H0v6c0 1.662 1.338 3 3 3h18c1.662 0 3-1.338 3-3v-1.393h-2.942c-1.092 0-1.986-.913-1.986-2.005v-.445c-4.046-.032-5.598-1.333-6.983-2.544-1.437-1.257-2.751-2.431-7.268-2.496zM21.058 15a.644.644 0 0 0-.647.66v1.942c0 .368.278.612.647.612H24V15z"/>`
    },
    caddy: {
      color: '#1F88C0',
      icon: `<path d="M11.094.47c-.842 0-1.696.092-2.552.288a11.37 11.37 0 0 0-4.87 2.423 10.632 10.632 0 0 0-2.36 2.826A10.132 10.132 0 0 0 .305 8.582c-.398 1.62-.4 3.336-.043 5.048.085.405.183.809.31 1.212a11.85 11.85 0 0 0 1.662 3.729 3.273 3.273 0 0 0-.086.427 3.323 3.323 0 0 0 2.848 3.71 3.279 3.279 0 0 0 1.947-.346c1.045.51 2.17.864 3.339 1.04a11.66 11.66 0 0 0 4.285-.155 11.566 11.566 0 0 0 4.936-2.485 10.643 10.643 0 0 0 2.352-2.894 11.164 11.164 0 0 0 1.356-4.424 11.214 11.214 0 0 0-.498-4.335c.175-.077.338-.175.486-.293a.444.444 89.992 0 0 .001 0c.402-.322.693-.794.777-1.342a2.146 2.146 0 0 0-1.79-2.434 2.115 2.115 0 0 0-1.205.171c-.038-.043-.078-.086-.113-.13a11.693 11.693 0 0 0-3.476-2.93 13.348 13.348 0 0 0-1.76-.81 13.55 13.55 0 0 0-2.06-.613A12.121 12.121 0 0 0 11.093.47Zm.714.328c.345-.004.688.01 1.028.042a9.892 9.892 0 0 1 2.743.639c.984.39 1.89.958 2.707 1.632.803.662 1.502 1.45 2.091 2.328.026.039.048.08.07.12a2.12 2.12 0 0 0-.435 2.646c-.158.114-.97.692-1.634 1.183-.414.308-.733.557-.733.557l.581.68s.296-.276.665-.638c.572-.562 1.229-1.233 1.395-1.403a2.122 2.122 0 0 0 1.907.677 11.229 11.229 0 0 1-.013 4.046 11.41 11.41 0 0 1-1.475 3.897 12.343 12.343 0 0 1-2.079 2.587c-1.19 1.125-2.633 2.022-4.306 2.531a10.826 10.826 0 0 1-3.973.484 11.04 11.04 0 0 1-3.057-.652 3.304 3.304 0 0 0 1.417-2.294 3.275 3.275 0 0 0-.294-1.842c.18-.162.403-.363.656-.6 1.015-.955 2.353-2.303 2.353-2.303l-.47-.599s-1.63.972-2.801 1.728c-.307.198-.573.378-.777.517a3.273 3.273 0 0 0-1.516-.611c-1.507-.198-2.927.672-3.487 2.017a10.323 10.323 0 0 1-.695-1.078A10.92 10.92 0 0 1 .728 14.8a10.35 10.35 0 0 1-.2-1.212c-.164-1.653.103-3.258.629-4.754a12.95 12.95 0 0 1 1.087-2.288c.57-.968 1.248-1.872 2.069-2.656A11.013 11.013 0 0 1 11.808.797Zm-.147 3.257a3.838 3.838 0 0 0-3.82 3.82v2.36h-.94c-.751 0-1.377.625-1.377 1.377v3.8h1.46v-3.718h9.354v6.264H10.02v1.46h6.4c.751 0 1.377-.625 1.377-1.377v-6.43c0-.751-.626-1.377-1.377-1.377h-.94v-2.36a3.838 3.838 0 0 0-3.82-3.819zm0 1.46a2.371 2.371 0 0 1 2.36 2.36v2.36H9.3v-2.36a2.372 2.372 0 0 1 2.36-2.36zm10.141.392a1.253 1.253 0 0 1 1.296 1.434c-.049.319-.217.59-.453.78-.266.213-.61.318-.968.264a1.253 1.253 0 0 1-1.045-1.42 1.255 1.255 0 0 1 1.17-1.058zM5.384 17.425a2.02 2.02 0 0 1 1.917 1.298c.116.3.159.628.114.967a2.015 2.015 0 0 1-2.249 1.728 2.016 2.016 0 0 1-1.727-2.25 2.017 2.017 0 0 1 1.945-1.743z"/>`
    }
  };

  function getServiceStyle(serviceId: string) {
    return serviceStyles[serviceId] || { color: '#6b7280', icon: '<circle cx="12" cy="12" r="10"/>' };
  }

  // Local binding for selected versions
  let localSelectedVersions = $state<Record<string, string>>({});

  // Sync with prop
  $effect(() => {
    localSelectedVersions = { ...selectedVersions };
  });

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  function handleDownload(serviceId: string) {
    const version = localSelectedVersions[serviceId];
    if (version) {
      onDownload(serviceId, version);
    }
  }
</script>

<div class="services-section">
  <div class="section-header">
    <h2>Services</h2>
  </div>

  <p class="section-description">
    Download and manage service binaries. Each service can have multiple versions installed.
  </p>

  <div class="services-grid">
    {#each serviceTypes as svc (svc.id)}
      {@const versions = installedVersions[svc.id] || []}
      {@const style = getServiceStyle(svc.id)}
      {@const isInstalled = versions.length > 0}
      <div class="service-card" class:installed={isInstalled}>
        <div class="card-icon" style="--brand-color: {style.color}">
          <svg viewBox="0 0 24 24" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
            {@html style.icon}
          </svg>
        </div>

        <div class="card-content">
          <div class="card-header">
            <h3 class="service-name">{svc.display_name}</h3>
            {#if svc.is_homebrew}
              <span class="homebrew-badge">Homebrew</span>
            {/if}
          </div>

          {#if isInstalled}
            <div class="versions-list">
              {#each versions as ver (ver)}
                <div class="version-tag">
                  <span>{ver}</span>
                  <button
                    class="version-delete"
                    onclick={() => onDeleteVersion(svc.id, ver)}
                    title="Delete version"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <span class="status-text">Not installed</span>
          {/if}
        </div>

        <div class="card-actions">
          {#if showVersionSelector[svc.id] && availableVersions[svc.id]?.length > 0}
            <div class="version-selector">
              <select
                bind:value={localSelectedVersions[svc.id]}
                class="version-dropdown"
              >
                {#each availableVersions[svc.id] as ver (ver.version)}
                  <option value={ver.version}>
                    {ver.version}{ver.label ? ` (${ver.label})` : ""}{ver.is_latest ? " - latest" : ""}
                  </option>
                {/each}
              </select>
              <div class="selector-buttons">
                <button
                  class="btn primary small"
                  onclick={() => handleDownload(svc.id)}
                  disabled={downloading[svc.id]}
                >
                  {svc.is_homebrew ? "Install" : "Download"}
                </button>
                <button
                  class="btn secondary small"
                  onclick={() => onCancelVersionSelector(svc.id)}
                >
                  Cancel
                </button>
              </div>
            </div>
          {:else}
            <button
              class="btn {isInstalled ? 'secondary' : 'primary'} small"
              onclick={() => onFetchVersions(svc.id)}
              disabled={downloading[svc.id] || loadingVersions[svc.id]}
            >
              {#if loadingVersions[svc.id]}
                Loading...
              {:else if downloading[svc.id]}
                {svc.is_homebrew ? "Installing..." : "Downloading..."}
              {:else if isInstalled}
                + Add Version
              {:else}
                {svc.is_homebrew ? "Install" : "Download"}
              {/if}
            </button>
          {/if}
        </div>

        {#if downloadProgress[svc.id]}
          {@const progress = downloadProgress[svc.id]}
          <div class="progress-section">
            {#if progress.phase === "extracting"}
              <div class="progress-bar indeterminate">
                <div class="progress-fill indeterminate-fill"></div>
              </div>
              <p class="progress-text">Extracting...</p>
            {:else if progress.phase === "installing" || progress.percentage < 0}
              <div class="progress-bar indeterminate">
                <div class="progress-fill indeterminate-fill"></div>
              </div>
              <p class="progress-text">Installing via Homebrew...</p>
            {:else if progress.total > 0}
              <div class="progress-bar">
                <div class="progress-fill" style="width: {progress.percentage}%"></div>
              </div>
              <p class="progress-text">
                {formatBytes(progress.downloaded)} / {formatBytes(progress.total)}
                ({progress.percentage.toFixed(1)}%)
              </p>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .services-section {
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

  .section-description {
    margin: 0;
    color: #86868b;
    font-size: 0.875rem;
  }

  .services-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1rem;
  }

  @media (max-width: 900px) {
    .services-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }

  @media (max-width: 600px) {
    .services-grid {
      grid-template-columns: 1fr;
    }
  }

  .service-card {
    background: white;
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    border: 1px solid #e5e5e5;
    transition: all 0.2s ease;
  }

  .service-card:hover {
    border-color: #d1d1d6;
  }

  .service-card.installed {
    border-color: rgba(52, 199, 89, 0.3);
  }

  @media (prefers-color-scheme: dark) {
    .service-card {
      background: #2c2c2e;
    }
    .service-card.installed {
      border-color: rgba(52, 199, 89, 0.4);
    }
  }

  .card-icon {
    width: 48px;
    height: 48px;
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--brand-color) 15%, transparent);
    color: var(--brand-color);
  }

  .card-icon svg {
    width: 28px;
    height: 28px;
  }

  .card-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .card-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .service-name {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
  }

  .homebrew-badge {
    font-size: 0.5625rem;
    padding: 0.125rem 0.375rem;
    background: #ff9500;
    color: white;
    border-radius: 3px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.025em;
  }

  .status-text {
    font-size: 0.8125rem;
    color: #86868b;
  }

  .versions-list {
    display: flex;
    flex-wrap: wrap;
    gap: 0.375rem;
  }

  .version-tag {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.1875rem 0.5rem;
    background: #34c759;
    color: white;
    border-radius: 100px;
    font-size: 0.6875rem;
    font-weight: 500;
    font-family: monospace;
  }

  .version-delete {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: rgba(255, 255, 255, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.15s ease;
    margin-left: 0.125rem;
  }

  .version-delete:hover {
    color: white;
  }

  .card-actions {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .version-selector {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .selector-buttons {
    display: flex;
    gap: 0.5rem;
  }

  .selector-buttons .btn {
    flex: 1;
  }

  .version-dropdown {
    width: 100%;
    padding: 0.5rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
    color: inherit;
  }

  @media (prefers-color-scheme: dark) {
    .version-dropdown {
      background: #1c1c1e;
      border-color: #3a3a3c;
    }
  }

  .progress-section {
    padding-top: 0.75rem;
    border-top: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .progress-section {
      border-top-color: #38383a;
    }
  }

  .progress-bar {
    width: 100%;
    height: 6px;
    background: #e5e5e5;
    border-radius: 3px;
    overflow: hidden;
  }

  @media (prefers-color-scheme: dark) {
    .progress-bar {
      background: #3a3a3c;
    }
  }

  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #ff6b6b, #ee5a24);
    transition: width 0.3s ease;
  }

  .progress-fill.indeterminate-fill {
    width: 30%;
    animation: indeterminate 1.5s infinite ease-in-out;
  }

  @keyframes indeterminate {
    0% { transform: translateX(-100%); }
    100% { transform: translateX(400%); }
  }

  .progress-text {
    font-size: 0.6875rem;
    color: #86868b;
    margin: 0.375rem 0 0;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    text-align: center;
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

  /* Light mode explicit overrides */
  :global(:root[data-theme="light"]) .service-card {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .btn.secondary {
    background: #e5e5e5 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .version-dropdown {
    background: white !important;
    border-color: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"]) .progress-section {
    border-top-color: #e5e5e5 !important;
  }

  /* Dark mode explicit overrides */
  :global(:root[data-theme="dark"]) .service-card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .version-dropdown {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
  }

  :global(:root[data-theme="dark"]) .progress-section {
    border-top-color: #38383a !important;
  }
</style>
