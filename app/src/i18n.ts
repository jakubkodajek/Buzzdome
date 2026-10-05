import { useEffect, useState } from "react";

export type Lang = "cs" | "en";

const STORAGE_KEY = "quiz-show.lang";

const strings = {
  cs: {
    appTitle: "Kvízová show",
    noShow: "Není spuštěná žádná show.",
    openQuiz: "Otevřít kvíz…",
    demoQuiz: "Ukázkový kvíz",
    resume: "Pokračovat v poslední show",
    resumeInfo: "{title} – {count} kroků",
    projector: "Projektor",
    openProjector: "Otevřít",
    closeProjector: "Zavřít",
    fullscreen: "Celá obrazovka",
    slide: "Snímek {n} / {total}",
    next: "Další",
    nextUp: "Následuje: {title}",
    right: "Správně",
    wrong: "Špatně",
    pause: "Pozastavit",
    resumeClock: "Pokračovat",
    endQuestion: "Ukončit otázku",
    leaderboard: "Pořadí",
    goTo: "Přejít na…",
    notes: "Poznámky",
    players: "Hráči",
    buzzer: "Tlačítko {n}",
    name: "Jméno",
    score: "Body",
    active: "Hraje",
    addPlayer: "Přidat hráče",
    virtualBuzzers: "Virtuální tlačítka",
    count: "Počet",
    keyboardHint: "Klávesy 1–0 = velké tlačítko hráčů 1–10. Mezerník = další, Y/N = správně/špatně, P = pauza, S = pořadí.",
    lobby: "Registrace",
    lobbyHint: "Stiskněte libovolné tlačítko na svém bzučáku",
    registered: "Přihlášeno: {n}",
    billboard: "Informační snímek",
    endOfRound: "Konec kola",
    ready: "Připraveno",
    running: "Běží",
    paused: "Pozastaveno",
    buzzed: "Bzučí: {name}",
    closed: "Uzavřeno",
    revealed: "Vyhodnoceno",
    final: "Konečné pořadí",
    answered: "Odpovědělo {n} z {total}",
    waitingForBuzz: "Kdo bude první?",
    timeUp: "Čas vypršel",
    points: "{n} b.",
    untimed: "Bez limitu",
    everyone: "Všichni odpovídají",
    fastestFinger: "Rychlá odpověď",
    correctAnswer: "Správná odpověď",
    drumroll: "A vítězem se stává…",
    place: "{n}. místo",
    language: "Jazyk",
  },
  en: {
    appTitle: "Quiz Show",
    noShow: "No show is running.",
    openQuiz: "Open quiz…",
    demoQuiz: "Demo quiz",
    resume: "Resume last show",
    resumeInfo: "{title} – {count} steps",
    projector: "Projector",
    openProjector: "Open",
    closeProjector: "Close",
    fullscreen: "Full screen",
    slide: "Slide {n} / {total}",
    next: "Next",
    nextUp: "Up next: {title}",
    right: "Right",
    wrong: "Wrong",
    pause: "Pause",
    resumeClock: "Resume",
    endQuestion: "End question",
    leaderboard: "Leaderboard",
    goTo: "Go to…",
    notes: "Notes",
    players: "Players",
    buzzer: "Buzzer {n}",
    name: "Name",
    score: "Score",
    active: "Playing",
    addPlayer: "Add player",
    virtualBuzzers: "Virtual buzzers",
    count: "Count",
    keyboardHint: "Keys 1–0 = big button of players 1–10. Space = next, Y/N = right/wrong, P = pause, S = leaderboard.",
    lobby: "Registration",
    lobbyHint: "Press any button on your buzzer",
    registered: "Registered: {n}",
    billboard: "Info slide",
    endOfRound: "End of round",
    ready: "Ready",
    running: "Running",
    paused: "Paused",
    buzzed: "Buzzed: {name}",
    closed: "Closed",
    revealed: "Revealed",
    final: "Final scores",
    answered: "{n} of {total} answered",
    waitingForBuzz: "Who's first?",
    timeUp: "Time's up",
    points: "{n} pts",
    untimed: "Untimed",
    everyone: "Everyone answers",
    fastestFinger: "Fastest finger",
    correctAnswer: "Correct answer",
    drumroll: "And the winner is…",
    place: "{n}. place",
    language: "Language",
  },
} as const;

export type StringKey = keyof (typeof strings)["cs"];

function readLang(): Lang {
  try {
    return localStorage.getItem(STORAGE_KEY) === "en" ? "en" : "cs";
  } catch {
    return "cs";
  }
}

export function useLang(): [Lang, (lang: Lang) => void] {
  const [lang, setLangState] = useState<Lang>(readLang);
  useEffect(() => {
    // Keeps the projector window in step with the Director.
    const onStorage = () => setLangState(readLang());
    window.addEventListener("storage", onStorage);
    return () => window.removeEventListener("storage", onStorage);
  }, []);
  const setLang = (next: Lang) => {
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      // Language then lasts only for this window.
    }
    setLangState(next);
  };
  return [lang, setLang];
}

export type Translate = (key: StringKey, params?: Record<string, string | number>) => string;

export function translator(lang: Lang): Translate {
  return (key, params) => {
    let text: string = strings[lang][key];
    for (const [name, value] of Object.entries(params ?? {})) {
      text = text.replace(`{${name}}`, String(value));
    }
    return text;
  };
}

export function playerName(t: Translate, player: { name: string; buzzer: number }): string {
  return player.name || t("buzzer", { n: player.buzzer });
}
