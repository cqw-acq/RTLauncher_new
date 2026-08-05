import type {
  LaunchAnalysisReport,
  LaunchLogEntry,
  LaunchStageTiming,
  Log4jLogEntry,
} from "@/types";
import type { AppLanguage } from "@/components/settings/settings-provider";
import type { AuthType } from "@/types";

const STAGE_ORDER = [
  "jvm_start",
  "loading_libraries",
  "loading_assets",
  "initializing_game",
  "loading_mods",
  "loading_world",
  "ready",
] as const;

const STAGE_NAMES: Record<(typeof STAGE_ORDER)[number], { zh: string; en: string }> = {
  jvm_start: { zh: "JVM 启动", en: "JVM Start" },
  loading_libraries: { zh: "加载库文件", en: "Loading Libraries" },
  loading_assets: { zh: "加载资源", en: "Loading Assets" },
  initializing_game: { zh: "初始化游戏", en: "Initializing Game" },
  loading_mods: { zh: "加载模组", en: "Loading Mods" },
  loading_world: { zh: "加载世界", en: "Loading World" },
  ready: { zh: "游戏就绪", en: "Game Ready" },
};

const STAGE_ENTRY_PATTERNS: Array<{ id: (typeof STAGE_ORDER)[number]; re: RegExp }> = [
  { id: "jvm_start", re: /Running with arguments:|Java HotSpot|OpenJDK|_JAVA_OPTIONS|Launching wrapped minecraft|Starting Minecraft|LiteLoader|Bootstrap/i },
  { id: "loading_libraries", re: /Loading libraries|Downloading library|Considering library|Library .* does not exist|Loaded \d+ libraries|Applying library|Tweaking classpath|ClassPath|Class path/i },
  { id: "loading_assets", re: /Loading assets|Reloading resources|Resource pack loading|Reloading ResourceManager|Applied.*resource pack|Assets loaded|Asset download|Loading locales|LanguageManager|texture atlas|Texture stitch|Loading model|Resource reload|Reloading.*resource|Preparing resource pack/i },
  { id: "initializing_game", re: /Initializing game|Starting game|Game instance created|Setting up game|Created.*dimensions|Game initialized|Starting integrated|Bootstrap|Minecraft .*starting|Minecraft main|Setting .*window|Creating window|Initializing NoFog|Launching Minecraft|Minecraft .* is starting|Loading mappings|Mapping set|MinecraftClient/i },
  { id: "loading_mods", re: /Loading mods|Mod loading|Forge mod loading|Fabric mod loading|Quilt mod loading|NeoForge mod loading|FML.*loading|FML:|ModLauncher|Found \d+ mods|Processing mods|Mod.*found|Applying mod|Mods loaded|Scanning.*mod|Mod list|Mod files|Mod containers|Constructing mods|Configuring mods|Loading plugin|Tweaker|Mod tweaker|Initialising mod|ModInitializer|onInitialize|ForgeModLoader|FMLAppLoader|PluginLoader|LiteLoader.*mod|tweaker class|ModValidator/i },
  { id: "loading_world", re: /Loading world|Preparing start region|Preparing spawn area|Time elapsed|Loading level|Reading.*level data|Building chunk|World loaded|Connecting to server|Integrated server|Starting integrated|Joining world|Spawn area|Chunk cache|Loading level.*level\.dat|Reading level\.dat|Level load|World init|Loading properties|Generated key for/i },
  { id: "ready", re: /Game started|Displaying screen|Main menu|Rendering screen|Opening screen|Started serving|Done .* for .*help|Help: |Title screen|GuiMainMenu|Init done|Initialization done|Minecraft initialized|Finished loading|Server started|Tick loop|MinecraftClient.*tick/i },
];

const STAGE_COMPLETION_PATTERNS: Array<{ id: (typeof STAGE_ORDER)[number]; re: RegExp }> = [
  { id: "jvm_start", re: /Loading libraries|Downloading library|Considering library|Tweaker|Applying tweak class|ModLauncher|Starting minecraft|Bootstrap/i },
  { id: "loading_libraries", re: /Loading assets|Reloading resources|Initializing game|Starting game|Reloading ResourceManager|Initializing NoFog|Loading mappings|Creating window|Asset|texture atlas/i },
  { id: "loading_assets", re: /Initializing game|Starting game|Game instance created|Loading mods|FML|ModLauncher|Initializing.*(mod|loader)|Loading mappings|MinecraftClient|Starting integrated/i },
  { id: "initializing_game", re: /Loading mods|Mod loading|Forge mod loading|Fabric mod loading|Quilt mod loading|NeoForge mod loading|FML.*loading|Loading world|Preparing start region|Constructing mods|Configuring mods|Mod.*found|Found \d+ mods|Processing.*mods|Mod list|Scanning.*mod|ModInitializer|onInitialize/i },
  { id: "loading_mods", re: /Loading world|Preparing start region|Preparing spawn area|Loading level|Reading.*level data|Game started|Displaying screen|Title screen|Main menu|GuiMainMenu|Mods loaded|Finished loading|Initialization done|MinecraftClient.*tick/i },
  { id: "loading_world", re: /Game started|Displaying screen|Main menu|Rendering screen|Opening screen|Started serving|Done .* for .*help|Help: /i },
];

const MOD_COUNT_RE = /Found[^\d]{0,60}(\d+)[^\d]{0,60}mods?|(\d+)[^\d]{0,20}of[^\d]{0,20}mods?|(?:loaded|loading|processing|listed|validated|scanned|constructed|configured)[^\d]{0,20}(\d+)[^\d]{0,20}mods?/i;
const MC_VERSION_RE = /Minecraft[^\d]{0,10}(\d+\.\d+(\.\d+)?)|MC[^\d]{0,6}(\d+\.\d+(\.\d+)?)|gameVersion\s*[=:]\s*(\d+\.\d+(\.\d+)?)|version[^\d]{0,6}(\d+\.\d+\.\d+)[^\d]{0,6}(?:minecraft|client|release)/i;
const FORGE_RE = /MinecraftForge|Forge Mod Loader|Forge version|net\.minecraftforge\b|fml(?:ml|common)?\b|FML:(?!.*userdev)|ModLauncher|cpw\.mods\.modlauncher|forge-|ForgeGradle|fmlclient|ForgeConfig|FMLConfig/i;
const FABRIC_RE = /Fabric Loader|fabric[- ]?loader|net\.fabricmc|fabric-api|FabricLoader|fabric-language-kotlin|FabricMC|Yarn|Quilted Fabric API|fabricloader/i;
const QUILT_RE = /Quilt Loader|quilt[- ]?loader|org\.quiltmc|QuiltMC|quilted.?fabric|quilt-loader|QuiltBase|QSL/i;
const NEOFORGE_RE = /NeoForge|NeoForgeModLoader|net\.neoforged|NeoForgeConfig|neoforge|NeoModLoader|FML2Provider|NeoForgeGradle|NeoForgeModValidator/i;
const LITELOADER_RE = /LiteLoader|com\.mumfrey\.liteloader|LiteMod|TweakClass|net\.mumfrey\b|LiteLoader\.|liteloader\.|litemod|liteloader-tweaker/i;

// OptiFine 不是 loader，但属于最常见的大型/冲突源，单独追踪版本与特有报错
const OPTIFINE_VERSION_RE = /OptiFine[_ ](\S+)|Optifine (?:HD )?(\S+)|OptiFine version[^\d]{0,6}(\S+)|Loading OptiFine|OptiFineClassTransformer|optifine\.|net\.optifine|Icaked .*OptiFine/i;

const FAILURE_PATTERNS: Array<{ re: RegExp; hint: { zh: string; en: string }; isAuthIssue?: boolean }> = [
  {
    re: /java\.lang\.OutOfMemoryError|java\.lang\.outofmemoryerror|GC overhead limit exceeded|Metaspace|Java heap space/i,
    hint: { zh: "内存不足：请增加 JVM 最大内存分配（-Xmx）", en: "Out of memory: increase the maximum JVM memory (-Xmx)." },
  },
  {
    re: /Could not reserve enough space for.*object heap|unable to create new native thread|initial heap size set to a larger value than the maximum heap size/i,
    hint: { zh: "JVM 无法分配堆内存：请减小内存设置或关闭其他程序", en: "JVM cannot reserve heap memory: lower the memory allocation or close other programs." },
  },
  {
    re: /UnsupportedClassVersionError|class file version.*wrong version|major\.minor version/i,
    hint: { zh: "Java 版本不匹配：请使用对应版本的 Java（旧版 Forge 需要 Java 8）", en: "Java version mismatch: use the correct JDK (older Forge requires Java 8)." },
  },
  {
    re: /Incompatible mods found|Some of your mods are incompatible|FormattedException.*incompatible|Provided.*mods? are incompatible/i,
    hint: { zh: "存在不兼容的模组：Fabric/Quilt 报告模组版本与游戏或彼此不兼容", en: "Incompatible mods detected: Fabric/Quilt reports mods incompatible with game or each other." },
  },
  {
    re: /mods? are? missing the required (?:language|provider|mod)|mod.*requires.*version|missing mods?|dependency.*not found|ModLoadingException|net\.minecraftforge\.fml\.common\.LoadingFailedException|needs? is required but it is not installed|没有安装它|but the mod.*but no (?:version|mod) is not? installed|(?:could not find required (?:mod|language|version))/i,
    hint: { zh: "存在缺失/不兼容的模组依赖：请检查并安装对应版本的依赖模组", en: "Missing or incompatible mod dependencies: install the required dependency mods with correct versions." },
  },
  {
    re: /DuplicateModsFoundException|duplicate mod|Mod duplicate encountered/i,
    hint: { zh: "检测到重复的模组文件：请清理 mods 目录", en: "Duplicate mods detected: clean up the mods folder." },
  },
  {
    re: /InvalidModuleDescriptorException|module not found|module.*does not read module/i,
    hint: { zh: "模组加载器/模块错误：检查加载器版本与模组兼容", en: "Module/loader error: verify loader version is compatible with mods." },
  },
  {
    re: /Mixin prepare failed|Mixin apply failed|mixin.*failed/i,
    hint: { zh: "Mixin 注入失败：常见于模组冲突或加载器不兼容", en: "Mixin injection failed: usually a mod conflict or incompatible loader." },
  },
  {
    re: /OpenGL|GLFW error|Cannot create window|pixel format|WGL|GLX|no GLX visuals/i,
    hint: { zh: "显卡/OpenGL 错误：请更新显卡驱动或关闭硬件冲突程序", en: "Graphics/OpenGL error: update GPU drivers or close conflicting programs." },
  },
  {
    re: /Connection refused|no such host is known|UnknownHostException|timed out|network is unreachable/i,
    hint: { zh: "网络错误：检查网络连接或登录服务器状态", en: "Network error: check your connection or authentication server status." },
  },
  {
    re: /Invalid access token|ForgeOAuth|401|403|authentication.*failed|login failed|Session ID mismatch|Token mismatch|UUID.*mismatch|failed to verify token|failed to login/i,
    hint: { zh: "登录凭证失效：请重新登录账户", en: "Auth token expired: sign in to your account again." },
    isAuthIssue: true,
  },
  {
    re: /The game crashed whilst|The game crashed|Unexpected error|Exception in server tick loop|Ticking|A fatal error has been detected by the Java Runtime Environment|EXCEPTION_ACCESS_VIOLATION/i,
    hint: { zh: "游戏崩溃：查看完整日志寻找具体错误信息", en: "Game crash: inspect the full log for specific details." },
  },
  {
    re: /Failed to load mod|加载模组失败|Caused by:.*(Exception|Error)/i,
    hint: { zh: "模组加载失败：可能需要更新加载器或修复模组间的冲突", en: "Failed to load a mod: you may need to update the loader or resolve mod conflicts." },
  },
  {
    re: /OptiFine.*(?:incompatible|conflict|error|exception|cannot load|class.*not found|ClassNotFoundException|NoSuchMethodError|IncompatibleClassChangeError)|optifine.*transformer.*fail|OptiFine shader pack|OptiFine incompatible with (?:Sodium|Rubidium|Iris|Embeddium|Oculus)|Sodium.*OptiFine|Rubidium.*OptiFine|can't load.*with OptiFine|OptiFine.*ModLoadingException|Forge.*OptiFine.*missing/i,
    hint: { zh: "OptiFine 不兼容：尝试关闭 Sodium/Rubidium，或使用 Iris+Oculus/Embeddium 替代 OptiFine", en: "OptiFine incompatibility: remove Sodium/Rubidium or use Iris+Oculus/Embeddium as an alternative." },
  },
  {
    re: /LiteLoader|TweakClass.*(?:not found|fail|error)|litemod.*(?:error|exception|fail|invalid|can't load)|LiteMod.*(?:exception|incompatible)/i,
    hint: { zh: "LiteLoader/Tweaker 相关错误：请检查 .litemod 文件与 LiteLoader/TweakClass 版本兼容", en: "LiteLoader/Tweaker error: verify .litemod files and LiteLoader/TweakClass versions." },
  },
  {
    re: /cpw\.mods\.modlauncher|IncompatibleClassChangeError|Bootstrap.*(?:error|exception|fail)|InvalidDist|Only in (?:server|client|dev)|java\.lang\.IllegalAccessError|IllegalStateException.*mixin|LoadingException|Could not find or load main class|java\.lang\.ClassNotFoundException.*(?:minecraft|loader|launcher|ModLauncher|FML|Forge|NeoForge|LiteLoader|fabric)/i,
    hint: { zh: "加载器启动失败：常见于 Forge/NeoForge/Fabric 版本不匹配、缺失或 launcher 启动参数错误", en: "Loader bootstrap failed: mismatched Forge/NeoForge/Fabric versions, missing files, or wrong launcher args." },
  },
  {
    re: /CoreMod.*(?:error|fail|exception|can't|cannot)|LoadingPlugin.*(?:fail|exception|error)|FML.*CORE|Forge Access|coremod|CoreModManager|InvalidCore|coremods?\b.*(?:invalid|missing|conflict)/i,
    hint: { zh: "Forge CoreMod/LoadingPlugin 错误：核心模组与 Forge/NeoForge 版本不匹配，请移除或更新", en: "Forge CoreMod/LoadingPlugin error: core mods mismatched with Forge/NeoForge version, remove or update them." },
  },
];

function createEmptyStages(language: AppLanguage): LaunchStageTiming[] {
  return STAGE_ORDER.map((id) => ({
    id,
    name: language === "en-US" ? STAGE_NAMES[id].en : STAGE_NAMES[id].zh,
    completed: false,
    enteredAt: null,
    completedAt: null,
    durationMs: null,
    logCount: 0,
  }));
}

/**
 * 从完整的启动日志流生成分析报告。
 *
 * 设计参考 HMCL：
 * - 按阶段记录进入/完成时间戳与日志量，生成阶段耗时分布
 * - 识别加载器、MC 版本、模组数量
 * - 统计警告/错误并匹配常见失败原因
 */
export function analyzeLaunchLogs(
  logs: LaunchLogEntry[],
  {
    language = "zh-CN",
    startedAt,
    endedAt,
    exitCode = null,
    finalStatus,
    accountType,
  }: {
    language?: AppLanguage;
    startedAt?: number | null;
    endedAt?: number | null;
    exitCode?: number | null;
    finalStatus?: LaunchAnalysisReport["finalStatus"];
    accountType?: AuthType;
  } = {},
): LaunchAnalysisReport {
  const stages = createEmptyStages(language);
  const stageByIndex = (i: number) => stages[i] ?? null;
  let currentStageIdx = -1;

  const ensureStageEntered = (idx: number, ts: number) => {
    const s = stageByIndex(idx);
    if (!s) return;
    if (s.enteredAt == null) s.enteredAt = ts;
    // 如果有更早的阶段还没标记完成，先一并闭合（按时间顺序）
    for (let j = Math.max(0, currentStageIdx); j < idx; j++) {
      const prev = stages[j];
      if (prev && !prev.completed && prev.enteredAt != null) {
        prev.completedAt = Math.max(prev.enteredAt, ts - 1);
        prev.completed = true;
        prev.durationMs = prev.completedAt - prev.enteredAt;
      }
    }
    currentStageIdx = idx;
  };

  const closeStage = (idx: number, ts: number) => {
    const s = stageByIndex(idx);
    if (!s) return;
    if (s.completed) return;
    if (s.enteredAt == null) s.enteredAt = ts;
    s.completedAt = ts;
    s.completed = true;
    s.durationMs = s.completedAt - s.enteredAt;
  };

  let warnCount = 0;
  let errorCount = 0;
  const errorSamples: string[] = [];
  let detectedModCount: number | null = null;
  let detectedMcVersion: string | null = null;
  let loader:
    | "Forge"
    | "Fabric"
    | "Quilt"
    | "NeoForge"
    | "LiteLoader"
    | "Vanilla"
    | null = null;
  let detectedOptifineVersion: string | null = null;
  let detectedAdditional: string[] = [];
  const log4jLogs: Log4jLogEntry[] = [];
  const seenLog4jMessages = new Set<string>();

  const parseLog4jLine = (message: string): { timestamp: string; level: string; logger?: string } | null => {
    const log4jRe = /\[(\d{2}:\d{2}:\d{2})\]\s*\[([^\]]+?)\]\s*\[([^\]]*?)\]:\s*/;
    const m = message.match(log4jRe);
    if (m) {
      return { timestamp: m[1], level: m[2].trim(), logger: m[3].trim() || undefined };
    }
    const log4j2Re = /\[(\d{2}:\d{2}:\d{2})(?:\.\d+)?\]\s*\[([^\]]+?)\]\s*\[([^\]]*?)\]\s*([A-Z]+)\s*:\s*/;
    const m2 = message.match(log4j2Re);
    if (m2) {
      return { timestamp: m2[1], level: m2[4], logger: m2[3] || undefined };
    }
    return null;
  };

  const extractLog4jMessage = (message: string): string => {
    const log4jRe = /\[[^\]]+\]\s*\[[^\]]+?\]\s*\[[^\]]*?\]:\s*/;
    return message.replace(log4jRe, "");
  };

  const isPremiumAccount = accountType === "microsoft";

  const matchesAnyFailurePattern = (message: string): string | null => {
    for (const { re, hint, isAuthIssue } of FAILURE_PATTERNS) {
      if (isAuthIssue && !isPremiumAccount) continue;
      if (re.test(message)) {
        return language === "en-US" ? hint.en : hint.zh;
      }
    }
    return null;
  };

  // 基准时间戳：优先使用传入的 startedAt，否则用当前时间（保证时间差恒为非负且稳定）
  const baseTs: number = startedAt ?? Date.now();

  for (let i = 0; i < logs.length; i++) {
    const log = logs[i];
    // 粗略的每条日志增量（实际时间戳通常存在于日志文本，这里退化为每条 1ms 用于排序）
    const approxTs = baseTs + i;

    if (currentStageIdx >= 0) {
      stages[currentStageIdx].logCount += 1;
    }

    if (log.level === "warn") warnCount++;
    if (log.level === "error") {
      errorCount++;
      if (errorSamples.length < 5) errorSamples.push(log.message);
    }

    const log4jInfo = parseLog4jLine(log.message);
    if (log4jInfo) {
      const problem = matchesAnyFailurePattern(log.message);
      const key = log.message.trim();
      if (!seenLog4jMessages.has(key)) {
        seenLog4jMessages.add(key);
        log4jLogs.push({
          timestamp: log4jInfo.timestamp,
          level: log4jInfo.level,
          message: extractLog4jMessage(log.message),
          logger: log4jInfo.logger,
          relatedProblem: problem ?? undefined,
        });
      }
    }

    for (let j = STAGE_ENTRY_PATTERNS.length - 1; j >= 0; j--) {
      const { id, re } = STAGE_ENTRY_PATTERNS[j];
      if (re.test(log.message)) {
        ensureStageEntered(j, approxTs);
        break;
      }
    }
    for (let j = 0; j < STAGE_COMPLETION_PATTERNS.length; j++) {
      const { id, re } = STAGE_COMPLETION_PATTERNS[j];
      if (re.test(log.message)) {
        closeStage(j, approxTs);
      }
    }

    const modMatch = log.message.match(MOD_COUNT_RE);
    if (modMatch) {
      // MOD_COUNT_RE 有多个捕获组：(\d+) 可能在第 1、2、3 组
      const nRaw = modMatch[1] ?? modMatch[2] ?? modMatch[3];
      if (nRaw) {
        const n = parseInt(nRaw, 10);
        if (!isNaN(n) && n >= 0 && (!detectedModCount || n > detectedModCount)) detectedModCount = n;
      }
    }

    const mcMatch = log.message.match(MC_VERSION_RE);
    if (mcMatch && !detectedMcVersion) {
      // MC_VERSION_RE 有 5 个捕获组，每个 mc 版本对应 (\d+\.\d+...) 位于 1、2、3、4 组的奇数位
      const v = mcMatch[1] ?? mcMatch[2] ?? mcMatch[3] ?? mcMatch[4] ?? mcMatch[5];
      if (v) detectedMcVersion = v;
    }

    if (!loader && FORGE_RE.test(log.message)) loader = "Forge";
    if (!loader && NEOFORGE_RE.test(log.message)) loader = "NeoForge";
    if (!loader && FABRIC_RE.test(log.message)) loader = "Fabric";
    if (!loader && QUILT_RE.test(log.message)) loader = "Quilt";
    if (!loader && LITELOADER_RE.test(log.message)) loader = "LiteLoader";

    const optMatch = log.message.match(OPTIFINE_VERSION_RE);
    if (optMatch) {
      const ov = optMatch[1] ?? optMatch[2] ?? optMatch[3];
      if (ov && !detectedOptifineVersion) detectedOptifineVersion = ov.replace(/[.,;:)\]'"!。]+$/g, "");
      if (!detectedAdditional.includes("OptiFine")) detectedAdditional.push("OptiFine");
    }
    // 额外标记常见冲突/依赖项
    if (/[\s'"(](Sodium|Rubidium|Iris|Oculus|Embeddium|Lithium|Starlight|C2ME|Concurrent Chunk Management Engine)[\s'")]/i.test(log.message)) {
      const m = log.message.match(/[\s'"(](Sodium|Rubidium|Iris|Oculus|Embeddium|Lithium|Starlight|C2ME|Concurrent Chunk Management Engine)[\s'")]/i);
      if (m && m[1] && !detectedAdditional.includes(m[1])) detectedAdditional.push(m[1]);
    }
  }

  // 如果到达最后一条日志，还未闭合的阶段根据 endedAt/当前估计时间进行结算
  const endTs = endedAt ?? baseTs + logs.length;
  for (let j = 0; j <= currentStageIdx; j++) {
    const s = stages[j];
    if (s && !s.completed && s.enteredAt != null) {
      s.completedAt = endTs;
      s.durationMs = s.completedAt - s.enteredAt;
      // 只有 ready 且有明确的结束标记才视为 completed，否则保留为 false 以示中断
      s.completed = j === stages.length - 1 ? currentStageIdx === stages.length - 1 && finalStatus === "running" : false;
    }
  }

  const detectedLoader = loader ?? (detectedModCount != null ? null : null);
  // 若没有模组信息也没有 loader，标记为原版
  let finalLoader: string | null =
    detectedLoader ?? (detectedModCount === 0 ? "Vanilla" : null);
  // 将 OptiFine 作为加载器附加信息（OptiFine 非 loader，但通常作为大型 mod 显示在 loader 旁）
  const extras = [];
  if (detectedOptifineVersion) extras.push(`OptiFine_${detectedOptifineVersion}`);
  else if (detectedAdditional.includes("OptiFine")) extras.push("OptiFine");
  for (const a of detectedAdditional) {
    if (a === "OptiFine") continue;
    extras.push(a);
  }
  if (extras.length > 0) {
    finalLoader = finalLoader ? `${finalLoader} · ${extras.join(" + ")}` : extras.join(" + ");
  }

  // 失败提示（结合语言选择）：优先匹配通用规则；额外从日志中提取具体的依赖需求行
  const failureHintsSet = new Set<string>();
  const DEPENDENCY_LINE_RES = [
    // 中文/英文："安装 X，从 ... 到 ... 的任意版本" / "Install X, any version from/to..."
    /-\s*安装[\s\S]{0,200}/i,
    /-\s*Install[\s\S]{0,200}/i,
    // 中文/英文："模组 'X' 需要 Y" / "Mod 'X' requires Y" / "...但没有安装它"
    /模组[\s\S]{0,160}(?:需要|缺少|但没有安装|但未找到)/i,
    /Mod[^\r\n]{0,160}(?:requires|missing|but no (?:mod|version|matching)|not installed)/i,
    // "需要 X" / "Missing required" / "Dependency not satisfied"
    /Missing required?[^\r\n]{0,160}/i,
    /Dependency[^\r\n]{0,160}(?:not satisfied|not found|missing|incompatible)/i,
    /需要[^\r\n]{0,120}(?:安装|版本|模组|依赖)/i,
    // 重复版本重复提示："Provided X mod is at version ... but we need"
    /Provided[^\r\n]{0,160}(?:mod|is at version|incompatible)/i,
  ];
  for (const log of logs) {
    const matchedHint = matchesAnyFailurePattern(log.message);
    if (matchedHint) {
      failureHintsSet.add(matchedHint);
    }
    for (const depRe of DEPENDENCY_LINE_RES) {
      let m: RegExpExecArray | null;
      // 多行搜索支持
      const multi = typeof log.message === "string" ? log.message.split(/\r?\n/) : [];
      for (const line of multi) {
        if ((m = depRe.exec(line))) {
          const trimmed = m[0].replace(/[\s\u0000-\u001F]+/g, " ").trim();
          if (trimmed.length >= 8 && trimmed.length <= 320) {
            failureHintsSet.add("· " + trimmed);
          }
        }
      }
    }
  }

  // 若发现 OptiFine + Sodium/Rubidium 同时出现，则补充一条冲突提示（不重复已有提示）
  if (detectedAdditional.includes("OptiFine") &&
      (detectedAdditional.includes("Sodium") || detectedAdditional.includes("Rubidium") || detectedAdditional.includes("Embeddium") || detectedAdditional.includes("Iris"))) {
    const tip = language === "en-US"
      ? "OptiFine is incompatible with Sodium/Rubidium/Iris/Embeddium: remove one or use Oculus+Iris (Fabric) or Embeddium (Forge/NeoForge)."
      : "OptiFine 与 Sodium/Rubidium/Iris/Embeddium 不兼容：请卸载其中一组，或使用 Iris+Oculus (Fabric) / Embeddium (Forge/NeoForge)。";
    if (!failureHintsSet.has(tip)) failureHintsSet.add(tip);
  }
  const failureHints = Array.from(failureHintsSet).slice(0, 8);

  const totalDurationMs =
    startedAt && endedAt ? endedAt - startedAt : stages.reduce<number>((acc, s) => acc + (s.durationMs ?? 0), 0) || null;

  // 若所有阶段都完成且 ready 日志出现，则判定 running
  let reportStatus: LaunchAnalysisReport["finalStatus"] = finalStatus ?? "in_progress";
  if (!reportStatus || reportStatus === "in_progress") {
    if (currentStageIdx === stages.length - 1) reportStatus = "running";
    if (errorCount > 0 && failureHints.length > 0) reportStatus = "error";
  }

  return {
    startedAt: startedAt ?? null,
    endedAt: endedAt ?? null,
    totalDurationMs,
    finalStatus: reportStatus,
    exitCode,
    stages,
    detectedModCount,
    detectedLoader: finalLoader,
    detectedMcVersion,
    warnCount,
    errorCount,
    errorSamples,
    failureHints,
    totalLogLines: logs.length,
    log4jLogs,
  };
}

export function formatDuration(ms: number | null): string {
  if (ms == null || ms < 0) return "—";
  if (ms < 1000) return `${ms} ms`;
  const s = Math.floor(ms / 1000);
  const rem = ms % 1000;
  const mm = Math.floor(s / 60);
  const ss = s % 60;
  if (mm === 0) return `${ss}.${String(Math.floor(rem / 100)).padStart(1, "0")}s`;
  return `${mm}m ${String(ss).padStart(2, "0")}s`;
}