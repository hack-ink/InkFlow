import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, MotionConfig, motion, type Variants } from "framer-motion";
import { Settings2 } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { setLiquidGlassEffect } from "tauri-plugin-liquid-glass-api";

const COLLAPSED_HEIGHT = 64;
const SETTINGS_PANEL_HEIGHT = 360;
const EXPANDED_HEIGHT = COLLAPSED_HEIGHT + SETTINGS_PANEL_HEIGHT;
const SETTINGS_PANEL_OPEN_MS = 260;
const SETTINGS_PANEL_CLOSE_MS = 260;
const SETTINGS_PANEL_VARIANTS: Variants = {
  closed: {
    height: 0,
    opacity: 1,
    y: -8,
    transition: { duration: SETTINGS_PANEL_CLOSE_MS / 1000, ease: [0.22, 1, 0.36, 1] },
  },
  open: {
    height: SETTINGS_PANEL_HEIGHT,
    opacity: 1,
    y: 0,
    transition: {
      duration: SETTINGS_PANEL_OPEN_MS / 1000,
      ease: [0.22, 1, 0.36, 1],
    },
  },
};

const SETTINGS_ITEM_VARIANTS: Variants = {
  closed: {
    opacity: 0,
    y: -6,
    transition: { duration: SETTINGS_PANEL_CLOSE_MS / 1000, ease: [0.22, 1, 0.36, 1] },
  },
  open: {
    opacity: 1,
    y: 0,
    transition: { duration: SETTINGS_PANEL_OPEN_MS / 1000, ease: [0.22, 1, 0.36, 1] },
  },
};

type SessionState =
  | "Hidden"
  | "Showing"
  | "Listening"
  | "Finalizing"
  | "Rewriting"
  | "RewriteReady"
  | "Injecting"
  | "Error";

type SessionStateEvent = {
  session_id: string;
  state: SessionState;
  reason?: string;
};

type SttPartialEvent = {
  session_id: string;
  revision: number;
  text: string;
  strategy?: "vad_chunk" | "sliding_window";
};

type SttFinalEvent = {
  session_id: string;
  text: string;
};

type LlmRewriteEvent = {
  session_id: string;
  text: string;
};

type ErrorEvent = {
  session_id?: string;
  code: string;
  message: string;
  recoverable: boolean;
};

type SettingsPublic = {
  llm: {
    base_url: string;
    model: string;
    temperature: number;
    system_prompt: string;
    has_api_key: boolean;
  };
  session: {
    silence_timeout_ms: number;
  };
  stt: {
    sherpa: {
      model_dir: string;
      provider: string;
      num_threads?: number | null;
      decoding_method: string;
      max_active_paths: number;
      rule1_min_trailing_silence: number;
      rule2_min_trailing_silence: number;
      rule3_min_utterance_length: number;
      prefer_int8: boolean;
      use_int8_decoder: boolean;
      chunk_ms: number;
    };
    whisper: {
      model_path: string;
      language: string;
      num_threads?: number | null;
      force_gpu?: boolean | null;
    };
    window: {
      enabled: boolean;
      window_ms: number;
      step_ms: number;
      context_ms: number;
      min_mean_abs: number;
      emit_every: number;
    };
    merge: {
      stable_ticks: number;
      rollback_threshold_tokens: number;
      overlap_k_words: number;
      overlap_k_chars: number;
    };
    profiles: {
      window_best_of: number;
      second_pass_best_of: number;
    };
  };
};

type SettingsPatch = {
  llm?: {
    base_url?: string;
    api_key?: string;
    model?: string;
    temperature?: number;
    system_prompt?: string;
  };
  session?: {
    silence_timeout_ms?: number;
  };
  stt?: {
    sherpa?: {
      model_dir?: string;
      provider?: string;
      num_threads?: number;
      decoding_method?: string;
      max_active_paths?: number;
      rule1_min_trailing_silence?: number;
      rule2_min_trailing_silence?: number;
      rule3_min_utterance_length?: number;
      prefer_int8?: boolean;
      use_int8_decoder?: boolean;
      chunk_ms?: number;
    };
    whisper?: {
      model_path?: string;
      language?: string;
      num_threads?: number;
      force_gpu?: boolean;
    };
    window?: {
      enabled?: boolean;
      window_ms?: number;
      step_ms?: number;
      context_ms?: number;
      min_mean_abs?: number;
      emit_every?: number;
    };
    merge?: {
      stable_ticks?: number;
      rollback_threshold_tokens?: number;
      overlap_k_words?: number;
      overlap_k_chars?: number;
    };
    profiles?: {
      window_best_of?: number;
      second_pass_best_of?: number;
    };
  };
};

type SessionAction =
  | { type: "show" }
  | { type: "start_new" }
  | { type: "enter" }
  | { type: "escape" }
  | { type: "rewrite" };

type EngineState = "ready" | "reloading" | "error";

type EngineStateEvent = {
  state: EngineState;
  reason?: string;
};

type EngineApplyResponse = {
  apply_level: "soft_applied" | "reloaded" | "restart_required";
  settings: SettingsPublic;
};

function App() {
  const [sessionState, setSessionState] = useState<SessionState>("Hidden");
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [reason, setReason] = useState<string | null>(null);
  const [rawText, setRawText] = useState<string>("");
  const [rewriteText, setRewriteText] = useState<string>("");
  const [error, setError] = useState<ErrorEvent | null>(null);
  const [sttDelta, setSttDelta] = useState<string>("");
  const [engineState, setEngineState] = useState<EngineState>("ready");
  const [engineReason, setEngineReason] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState<boolean>(false);
  const [settings, setSettings] = useState<SettingsPublic | null>(null);
  const [settingsSaving, setSettingsSaving] = useState<boolean>(false);

  const [baseUrlDraft, setBaseUrlDraft] = useState<string>("");
  const [modelDraft, setModelDraft] = useState<string>("");
  const [temperatureDraft, setTemperatureDraft] = useState<string>("0.2");
  const [systemPromptDraft, setSystemPromptDraft] = useState<string>("");
  const [apiKeyDraft, setApiKeyDraft] = useState<string>("");
  const [clearApiKey, setClearApiKey] = useState<boolean>(false);
  const [silenceTimeoutDraft, setSilenceTimeoutDraft] = useState<string>("2500");

  const [sherpaModelDirDraft, setSherpaModelDirDraft] = useState<string>("");
  const [sherpaProviderDraft, setSherpaProviderDraft] = useState<string>("cpu");
  const [sherpaThreadsDraft, setSherpaThreadsDraft] = useState<string>("");
  const [sherpaChunkMsDraft, setSherpaChunkMsDraft] = useState<string>("170");

  const [whisperModelPathDraft, setWhisperModelPathDraft] = useState<string>("");
  const [whisperLanguageDraft, setWhisperLanguageDraft] = useState<string>("en");
  const [whisperThreadsDraft, setWhisperThreadsDraft] = useState<string>("");
  const [whisperGpuDraft, setWhisperGpuDraft] = useState<string>("default");

  const [windowEnabledDraft, setWindowEnabledDraft] = useState<boolean>(true);
  const [windowMsDraft, setWindowMsDraft] = useState<string>("4000");
  const [windowStepMsDraft, setWindowStepMsDraft] = useState<string>("400");
  const [windowContextMsDraft, setWindowContextMsDraft] = useState<string>("800");
  const [windowMinMeanAbsDraft, setWindowMinMeanAbsDraft] = useState<string>("0.001");
  const [windowEmitEveryDraft, setWindowEmitEveryDraft] = useState<string>("1");

  const [windowBestOfDraft, setWindowBestOfDraft] = useState<string>("1");
  const [secondPassBestOfDraft, setSecondPassBestOfDraft] = useState<string>("5");

  const [stableTicksDraft, setStableTicksDraft] = useState<string>("3");
  const [rollbackThresholdDraft, setRollbackThresholdDraft] = useState<string>("8");
  const [overlapWordsDraft, setOverlapWordsDraft] = useState<string>("30");
  const [overlapCharsDraft, setOverlapCharsDraft] = useState<string>("100");

  const currentSessionRef = useRef<string | null>(null);
  const sttRevisionRef = useRef<number>(0);
  const lastRawTextRef = useRef<string>("");
  const pttActiveRef = useRef<boolean>(false);
  const engineStateRef = useRef<EngineState>("ready");
  const transcriptRef = useRef<HTMLDivElement | null>(null);
  const shouldAutoScrollRef = useRef<boolean>(true);
  const resizeTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    let disposed = false;

    const applySystemGlass = async () => {
      try {
        if (!navigator.userAgent.includes("Mac")) {
          return;
        }

        if (disposed) {
          return;
        }

        await setLiquidGlassEffect({ cornerRadius: 16 });
        if (!disposed) {
          document.documentElement.dataset.glass = "system";
        }
      } catch (err: unknown) {
        console.warn("Failed to apply system glass effect.", err);
      }
    };

    void applySystemGlass();

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenSttPartial: (() => void) | undefined;
    let unlistenSttFinal: (() => void) | undefined;
    let unlistenRewrite: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;
    let unlistenEngineState: (() => void) | undefined;

    listen<SessionStateEvent>("session/state", (event) => {
      if (currentSessionRef.current !== event.payload.session_id) {
        currentSessionRef.current = event.payload.session_id;
        setRawText("");
        setRewriteText("");
        setSttDelta("");
        sttRevisionRef.current = 0;
        lastRawTextRef.current = "";
      }

      setSessionId(event.payload.session_id);
      setSessionState(event.payload.state);
      setReason(event.payload.reason ?? null);
      setError(null);
      if (event.payload.state === "Hidden") {
        currentSessionRef.current = null;
        setRawText("");
        setRewriteText("");
        setSttDelta("");
        sttRevisionRef.current = 0;
        lastRawTextRef.current = "";
        pttActiveRef.current = false;
        setSettingsOpen(false);
      }
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlisten = fn;
    });

    listen<SttPartialEvent>("stt/partial", (event) => {
      const activeSessionId = currentSessionRef.current;
      if (!activeSessionId) return;
      if (event.payload.session_id !== activeSessionId) return;
      if (event.payload.revision <= sttRevisionRef.current) return;

      const prevText = lastRawTextRef.current;
      const nextText = event.payload.text;
      sttRevisionRef.current = event.payload.revision;
      lastRawTextRef.current = nextText;

      setRawText(nextText);
      setSttDelta(nextText.startsWith(prevText) ? nextText.slice(prevText.length) : "");
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlistenSttPartial = fn;
    });

    listen<SttFinalEvent>("stt/final", (event) => {
      const activeSessionId = currentSessionRef.current;
      if (!activeSessionId) return;
      if (event.payload.session_id !== activeSessionId) return;
      setRawText(event.payload.text);
      setSttDelta("");
      lastRawTextRef.current = event.payload.text;
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlistenSttFinal = fn;
    });

    listen<LlmRewriteEvent>("llm/rewrite", (event) => {
      const activeSessionId = currentSessionRef.current;
      if (!activeSessionId) return;
      if (event.payload.session_id !== activeSessionId) return;
      setRewriteText(event.payload.text);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlistenRewrite = fn;
    });

    listen<ErrorEvent>("error", (event) => {
      const activeSessionId = currentSessionRef.current;
      if (event.payload.session_id && event.payload.session_id !== activeSessionId) return;
      setError(event.payload);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlistenError = fn;
    });

    listen<EngineStateEvent>("engine/state", (event) => {
      setEngineState(event.payload.state);
      setEngineReason(event.payload.reason ?? null);
    }).then((fn) => {
      if (disposed) {
        fn();
        return;
      }
      unlistenEngineState = fn;
    });

    return () => {
      disposed = true;
      if (unlisten) unlisten();
      if (unlistenSttPartial) unlistenSttPartial();
      if (unlistenSttFinal) unlistenSttFinal();
      if (unlistenRewrite) unlistenRewrite();
      if (unlistenError) unlistenError();
      if (unlistenEngineState) unlistenEngineState();
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        pttActiveRef.current = false;
        void invoke("session_dispatch", { action: { type: "escape" } satisfies SessionAction });
      }

      if (e.code === "Space") {
        if (engineStateRef.current !== "ready") return;
        const target = e.target as HTMLElement | null;
        const tagName = target?.tagName ?? "";
        if (tagName === "INPUT" || tagName === "TEXTAREA" || tagName === "SELECT") return;
        if ((target as HTMLElement | null)?.isContentEditable) return;

        e.preventDefault();
        if (e.repeat) return;
        if (pttActiveRef.current) return;
        pttActiveRef.current = true;
        void invoke("session_dispatch", { action: { type: "start_new" } satisfies SessionAction });
        return;
      }

      if (e.key === "Enter") {
        void invoke("session_dispatch", { action: { type: "enter" } satisfies SessionAction });
      }
    };

    const onKeyUp = (e: KeyboardEvent) => {
      if (e.code !== "Space") return;

      const target = e.target as HTMLElement | null;
      const tagName = target?.tagName ?? "";
      if (tagName === "INPUT" || tagName === "TEXTAREA" || tagName === "SELECT") return;
      if ((target as HTMLElement | null)?.isContentEditable) return;

      e.preventDefault();
      if (!pttActiveRef.current) return;
      pttActiveRef.current = false;
      void invoke("session_dispatch", { action: { type: "enter" } satisfies SessionAction });
    };

    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, []);

  useEffect(() => {
    engineStateRef.current = engineState;
  }, [engineState]);

  useEffect(() => {
    if (!settingsOpen) return;

    invoke<SettingsPublic>("settings_get")
      .then((s) => {
        setSettings(s);
        setBaseUrlDraft(s.llm.base_url);
        setModelDraft(s.llm.model);
        setTemperatureDraft(String(s.llm.temperature));
        setSystemPromptDraft(s.llm.system_prompt);
        setSilenceTimeoutDraft(String(s.session.silence_timeout_ms));

        setSherpaModelDirDraft(s.stt.sherpa.model_dir);
        setSherpaProviderDraft(s.stt.sherpa.provider);
        setSherpaThreadsDraft(s.stt.sherpa.num_threads != null ? String(s.stt.sherpa.num_threads) : "");
        setSherpaChunkMsDraft(String(s.stt.sherpa.chunk_ms));

        setWhisperModelPathDraft(s.stt.whisper.model_path);
        setWhisperLanguageDraft(s.stt.whisper.language);
        setWhisperThreadsDraft(s.stt.whisper.num_threads != null ? String(s.stt.whisper.num_threads) : "");
        setWhisperGpuDraft(
          s.stt.whisper.force_gpu == null ? "default" : s.stt.whisper.force_gpu ? "on" : "off",
        );

        setWindowEnabledDraft(s.stt.window.enabled);
        setWindowMsDraft(String(s.stt.window.window_ms));
        setWindowStepMsDraft(String(s.stt.window.step_ms));
        setWindowContextMsDraft(String(s.stt.window.context_ms));
        setWindowMinMeanAbsDraft(String(s.stt.window.min_mean_abs));
        setWindowEmitEveryDraft(String(s.stt.window.emit_every));

        setWindowBestOfDraft(String(s.stt.profiles.window_best_of));
        setSecondPassBestOfDraft(String(s.stt.profiles.second_pass_best_of));

        setStableTicksDraft(String(s.stt.merge.stable_ticks));
        setRollbackThresholdDraft(String(s.stt.merge.rollback_threshold_tokens));
        setOverlapWordsDraft(String(s.stt.merge.overlap_k_words));
        setOverlapCharsDraft(String(s.stt.merge.overlap_k_chars));
        setApiKeyDraft("");
        setClearApiKey(false);
      })
      .catch((err: unknown) => {
        setError({
          code: "settings_get_failed",
          message: String(err),
          recoverable: true,
        });
      });
  }, [settingsOpen]);

  useEffect(() => {
    const targetHeight = settingsOpen ? EXPANDED_HEIGHT : COLLAPSED_HEIGHT;
    if (resizeTimeoutRef.current != null) {
      window.clearTimeout(resizeTimeoutRef.current);
      resizeTimeoutRef.current = null;
    }

    if (settingsOpen) {
      void invoke("overlay_set_height", { height: targetHeight, animate: false }).catch((err: unknown) => {
        console.warn("Failed to resize the overlay window.", err);
      });
      return;
    }

    resizeTimeoutRef.current = window.setTimeout(() => {
      void invoke("overlay_set_height", { height: targetHeight, animate: false }).catch((err: unknown) => {
        console.warn("Failed to resize the overlay window.", err);
      });
    }, SETTINGS_PANEL_CLOSE_MS);
  }, [settingsOpen]);

  useEffect(() => {
    return () => {
      if (resizeTimeoutRef.current != null) {
        window.clearTimeout(resizeTimeoutRef.current);
      }
    };
  }, []);

  const onSaveSettings = async () => {
    setSettingsSaving(true);
    try {
      const whisperForceGpu =
        whisperGpuDraft === "default" ? undefined : whisperGpuDraft === "on" ? true : false;

      const patch: SettingsPatch = {
        llm: {
          base_url: baseUrlDraft,
          model: modelDraft,
          temperature: Number(temperatureDraft),
          system_prompt: systemPromptDraft,
        },
        session: {
          silence_timeout_ms: Number(silenceTimeoutDraft),
        },
        stt: {
          sherpa: {
            model_dir: sherpaModelDirDraft,
            provider: sherpaProviderDraft,
            num_threads: sherpaThreadsDraft.trim().length > 0 ? Number(sherpaThreadsDraft) : undefined,
            chunk_ms: Number(sherpaChunkMsDraft),
          },
          whisper: {
            model_path: whisperModelPathDraft,
            language: whisperLanguageDraft,
            num_threads: whisperThreadsDraft.trim().length > 0 ? Number(whisperThreadsDraft) : undefined,
            force_gpu: whisperForceGpu,
          },
          window: {
            enabled: windowEnabledDraft,
            window_ms: Number(windowMsDraft),
            step_ms: Number(windowStepMsDraft),
            context_ms: Number(windowContextMsDraft),
            min_mean_abs: Number(windowMinMeanAbsDraft),
            emit_every: Number(windowEmitEveryDraft),
          },
          profiles: {
            window_best_of: Number(windowBestOfDraft),
            second_pass_best_of: Number(secondPassBestOfDraft),
          },
          merge: {
            stable_ticks: Number(stableTicksDraft),
            rollback_threshold_tokens: Number(rollbackThresholdDraft),
            overlap_k_words: Number(overlapWordsDraft),
            overlap_k_chars: Number(overlapCharsDraft),
          },
        },
      };

      if (clearApiKey) patch.llm!.api_key = "";
      if (!clearApiKey && apiKeyDraft.trim().length > 0) patch.llm!.api_key = apiKeyDraft.trim();

      const applied = await invoke<EngineApplyResponse>("engine_apply_settings", { patch });
      setSettings(applied.settings);
      setApiKeyDraft("");
      setClearApiKey(false);
      setSettingsOpen(false);
    } catch (err: unknown) {
      setError({
        code: "engine_apply_settings_failed",
        message: String(err),
        recoverable: true,
      });
    } finally {
      setSettingsSaving(false);
    }
  };

  const onOpenMicrophoneSettings = () => {
    void invoke("platform_open_system_settings", { target: "microphone" });
  };

  const onOpenAccessibilitySettings = () => {
    void invoke("platform_open_system_settings", { target: "accessibility" });
  };

  const displayText = useMemo(() => {
    if (sessionState === "RewriteReady" || sessionState === "Injecting") {
      return rewriteText || rawText;
    }
    return rawText;
  }, [rawText, rewriteText, sessionState]);

  const delta = sessionState === "Listening" ? sttDelta : "";
  const prefix = delta ? displayText.slice(0, Math.max(0, displayText.length - delta.length)) : displayText;

  const isMicrophoneError = error?.code.startsWith("microphone") ?? false;
  const isAccessibilityError = error?.code === "accessibility_permission_required";

  const displayLine = useMemo(() => {
    if (displayText.trim().length > 0) return displayText;
    if (error) return error.message || error.code;
    if (engineState === "reloading") return engineReason || "Loading speech engine...";
    if (engineState === "error") return engineReason || "Speech engine error.";
    if (reason) return reason;
    return "Hold Space to talk, and release to stop. Press Enter to inject.";
  }, [displayText, engineReason, engineState, error, reason]);

  const updateTranscriptScrollState = () => {
    const node = transcriptRef.current;
    if (!node) {
      return;
    }

    const atEnd = node.scrollWidth - node.scrollLeft - node.clientWidth <= 4;
    shouldAutoScrollRef.current = atEnd;
  };

  useEffect(() => {
    const node = transcriptRef.current;
    if (!node) {
      return;
    }

    if (shouldAutoScrollRef.current) {
      node.scrollLeft = node.scrollWidth;
    }
    updateTranscriptScrollState();
  }, [displayLine, delta]);

  return (
    <MotionConfig reducedMotion="never">
      <div className="h-full w-full">
        <div className="glass-shell h-full w-full transform-gpu rounded-2xl">
          <div className="glass-content relative h-full w-full">
            <div className="relative z-10 flex h-16 items-center gap-3 px-5">
              <motion.div
                animate={sessionState === "Listening" ? { scale: [1, 1.25, 1] } : { scale: 1 }}
                transition={{ duration: 1.2, repeat: sessionState === "Listening" ? Infinity : 0 }}
                className="h-2.5 w-2.5 rounded-full bg-emerald-400 shadow-[0_0_0_6px_rgba(52,211,153,0.12)]"
              />

              <div className="min-w-0 flex-1">
                <div className="flex h-8 items-center">
                  <div
                    ref={transcriptRef}
                    onScroll={updateTranscriptScrollState}
                    className="glass-transcript-scroll-x h-full text-[15px] leading-5 text-white/90"
                    title={displayLine}
                  >
                    {displayText.trim().length > 0 ? (
                      <>
                        <span className="glass-contrast-text">{prefix}</span>
                        {delta ? (
                          <motion.span
                            key={`${sessionId ?? "none"}:${sttRevisionRef.current}`}
                            initial={{ opacity: 0, filter: "blur(4px)" }}
                            animate={{ opacity: 1, filter: "blur(0px)" }}
                            transition={{ duration: 0.18 }}
                            className="rounded-md bg-white/10 px-1 py-0.5"
                          >
                            <span className="glass-contrast-text">{delta}</span>
                          </motion.span>
                        ) : null}
                      </>
                    ) : error ? (
                      <span className="text-red-100/90">{displayLine}</span>
                    ) : (
                      <span className="glass-contrast-text text-white/55">{displayLine}</span>
                    )}
                  </div>
                </div>
              </div>

              {sessionState === "RewriteReady" ? (
                <button
                  type="button"
                  onClick={() =>
                    void invoke("session_dispatch", { action: { type: "rewrite" } satisfies SessionAction })
                  }
                  className="glass-button h-8 px-3 text-xs"
                >
                  <span className="glass-contrast-text">Rewrite</span>
                </button>
              ) : null}

              <button
                type="button"
                onClick={() => setSettingsOpen((open) => !open)}
                className={`glass-icon-button ${settingsOpen ? "glass-icon-button--active" : ""}`}
                aria-label="Toggle settings"
                aria-expanded={settingsOpen}
              >
                <Settings2 className="glass-contrast-text h-4 w-4" />
              </button>
            </div>

            <AnimatePresence>
              {settingsOpen ? (
                <motion.div
                  key="settings"
                  variants={SETTINGS_PANEL_VARIANTS}
                  initial="closed"
                  animate="open"
                  exit="closed"
                  className="glass-panel absolute left-0 right-0 top-16 overflow-hidden border-t border-white/10"
                >
                  <div className="h-full overflow-y-auto px-5 pb-4 pt-3">
                  {error ? (
                    <motion.div
                      variants={SETTINGS_ITEM_VARIANTS}
                      className="mb-3 rounded-xl border border-red-500/20 bg-red-500/10 px-3 py-2 text-sm text-red-100/90"
                    >
                      <div className="font-medium">{error.code}</div>
                      <div className="mt-1 text-xs leading-5 text-red-100/80">{error.message}</div>
                      {isMicrophoneError ? (
                        <button
                          type="button"
                          onClick={onOpenMicrophoneSettings}
                          className="glass-button mt-2 rounded-lg px-3 py-1.5 text-xs"
                        >
                          Open Microphone Settings
                        </button>
                      ) : null}
                      {isAccessibilityError ? (
                        <button
                          type="button"
                          onClick={onOpenAccessibilitySettings}
                          className="glass-button mt-2 rounded-lg px-3 py-1.5 text-xs"
                        >
                          Open Accessibility Settings
                        </button>
                      ) : null}
                    </motion.div>
                  ) : null}

                  <motion.div variants={SETTINGS_ITEM_VARIANTS} className="mt-1 grid grid-cols-2 gap-3">
                    <label className="block">
                      <div className="glass-label glass-contrast-text">Base URL</div>
                      <input
                        value={baseUrlDraft}
                        onChange={(e) => setBaseUrlDraft(e.target.value)}
                        placeholder="https://api.openai.com/v1"
                        className="glass-input"
                      />
                    </label>
                    <label className="block">
                      <div className="glass-label glass-contrast-text">Model</div>
                      <input
                        value={modelDraft}
                        onChange={(e) => setModelDraft(e.target.value)}
                        placeholder="gpt-4o-mini"
                        className="glass-input"
                      />
                    </label>
                  </motion.div>

                  <motion.div variants={SETTINGS_ITEM_VARIANTS}>
                    <label className="block">
                      <div className="flex items-center justify-between">
                        <div className="glass-label glass-contrast-text">API key</div>
                        <div className="glass-meta glass-contrast-text">
                          {settings?.llm.has_api_key ? "Saved" : "Not set"}
                        </div>
                      </div>
                      <input
                        value={apiKeyDraft}
                        onChange={(e) => setApiKeyDraft(e.target.value)}
                        placeholder="sk-..."
                        className="glass-input"
                      />
                      <label className="mt-2 flex items-center gap-2 text-xs text-white/60">
                        <input type="checkbox" checked={clearApiKey} onChange={(e) => setClearApiKey(e.target.checked)} />
                        <span className="glass-contrast-text">Clear the stored API key.</span>
                      </label>
                    </label>
                  </motion.div>

                  <motion.div variants={SETTINGS_ITEM_VARIANTS}>
                    <details className="group">
                      <summary className="glass-summary glass-contrast-text cursor-pointer">Advanced</summary>
                      <div className="mt-3 space-y-3">
                        <label className="block">
                          <div className="glass-label glass-contrast-text">
                            Temperature
                          </div>
                          <input
                            value={temperatureDraft}
                            onChange={(e) => setTemperatureDraft(e.target.value)}
                            inputMode="decimal"
                            className="glass-input"
                          />
                        </label>
                        <label className="block">
                          <div className="glass-label glass-contrast-text">
                            Silence timeout (ms)
                          </div>
                          <input
                            value={silenceTimeoutDraft}
                            onChange={(e) => setSilenceTimeoutDraft(e.target.value)}
                            inputMode="numeric"
                            className="glass-input"
                          />
                        </label>
                        <label className="block">
                          <div className="glass-label glass-contrast-text">
                            System prompt
                          </div>
                          <textarea
                            value={systemPromptDraft}
                            onChange={(e) => setSystemPromptDraft(e.target.value)}
                            rows={5}
                            className="glass-textarea"
                          />
                        </label>
                      </div>
                    </details>
                  </motion.div>

                  <motion.div variants={SETTINGS_ITEM_VARIANTS}>
                    <details className="group">
                      <summary className="glass-summary glass-contrast-text cursor-pointer">Speech engine</summary>
                      <div className="mt-3 space-y-4">
                        <div className="grid grid-cols-2 gap-3">
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Sherpa model dir
                            </div>
                            <input
                              value={sherpaModelDirDraft}
                              onChange={(e) => setSherpaModelDirDraft(e.target.value)}
                              placeholder="Auto"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Sherpa provider
                            </div>
                            <input
                              value={sherpaProviderDraft}
                              onChange={(e) => setSherpaProviderDraft(e.target.value)}
                              placeholder="cpu"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Sherpa threads
                            </div>
                            <input
                              value={sherpaThreadsDraft}
                              onChange={(e) => setSherpaThreadsDraft(e.target.value)}
                              inputMode="numeric"
                              placeholder="Default"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Sherpa chunk (ms)
                            </div>
                            <input
                              value={sherpaChunkMsDraft}
                              onChange={(e) => setSherpaChunkMsDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                        </div>

                        <div className="grid grid-cols-2 gap-3">
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Whisper model path
                            </div>
                            <input
                              value={whisperModelPathDraft}
                              onChange={(e) => setWhisperModelPathDraft(e.target.value)}
                              placeholder="Auto"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Whisper language
                            </div>
                            <input
                              value={whisperLanguageDraft}
                              onChange={(e) => setWhisperLanguageDraft(e.target.value)}
                              placeholder="en, zh, auto"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Whisper threads
                            </div>
                            <input
                              value={whisperThreadsDraft}
                              onChange={(e) => setWhisperThreadsDraft(e.target.value)}
                              inputMode="numeric"
                              placeholder="Default"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Whisper GPU
                            </div>
                            <select
                              value={whisperGpuDraft}
                              onChange={(e) => setWhisperGpuDraft(e.target.value)}
                              className="glass-select"
                            >
                              <option value="default">Default</option>
                              <option value="on">Force on</option>
                              <option value="off">Force off</option>
                            </select>
                          </label>
                        </div>

                        <label className="flex items-center gap-2 text-xs text-white/70">
                          <input
                            type="checkbox"
                            checked={windowEnabledDraft}
                            onChange={(e) => setWindowEnabledDraft(e.target.checked)}
                          />
                          <span className="glass-contrast-text">Enable sliding-window refinement.</span>
                        </label>

                        <div className="grid grid-cols-2 gap-3">
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Window (ms)
                            </div>
                            <input
                              value={windowMsDraft}
                              onChange={(e) => setWindowMsDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Step (ms)
                            </div>
                            <input
                              value={windowStepMsDraft}
                              onChange={(e) => setWindowStepMsDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Context (ms)
                            </div>
                            <input
                              value={windowContextMsDraft}
                              onChange={(e) => setWindowContextMsDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Min mean abs
                            </div>
                            <input
                              value={windowMinMeanAbsDraft}
                              onChange={(e) => setWindowMinMeanAbsDraft(e.target.value)}
                              inputMode="decimal"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Emit every
                            </div>
                            <input
                              value={windowEmitEveryDraft}
                              onChange={(e) => setWindowEmitEveryDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Window best_of
                            </div>
                            <input
                              value={windowBestOfDraft}
                              onChange={(e) => setWindowBestOfDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Second-pass best_of
                            </div>
                            <input
                              value={secondPassBestOfDraft}
                              onChange={(e) => setSecondPassBestOfDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                        </div>

                        <div className="grid grid-cols-2 gap-3">
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Stable ticks
                            </div>
                            <input
                              value={stableTicksDraft}
                              onChange={(e) => setStableTicksDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Rollback threshold
                            </div>
                            <input
                              value={rollbackThresholdDraft}
                              onChange={(e) => setRollbackThresholdDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Overlap K (words)
                            </div>
                            <input
                              value={overlapWordsDraft}
                              onChange={(e) => setOverlapWordsDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                          <label className="block">
                            <div className="glass-label glass-contrast-text">
                              Overlap K (chars)
                            </div>
                            <input
                              value={overlapCharsDraft}
                              onChange={(e) => setOverlapCharsDraft(e.target.value)}
                              inputMode="numeric"
                              className="glass-input"
                            />
                          </label>
                        </div>
                      </div>
                    </details>
                  </motion.div>

                  <motion.div variants={SETTINGS_ITEM_VARIANTS} className="mt-4 flex items-center justify-end gap-2">
                    <button
                      type="button"
                      onClick={onSaveSettings}
                      disabled={settingsSaving}
                      className="glass-button glass-button--primary px-4 py-2 text-xs font-medium disabled:opacity-60"
                    >
                      <span className="glass-contrast-text">{settingsSaving ? "Saving..." : "Save"}</span>
                    </button>
                  </motion.div>
                  </div>
                </motion.div>
              ) : null}
            </AnimatePresence>
          </div>
        </div>
      </div>
    </MotionConfig>
  );
}

export default App
