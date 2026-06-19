import { createContext, useContext, useMemo, type ReactNode } from "react";
import { createI18n, type LanguageChoice } from "./catalog";

type I18nValue = ReturnType<typeof createI18n>;

const I18nContext = createContext<I18nValue>(createI18n("zh-CN"));

type I18nProviderProps = {
  children: ReactNode;
  language?: LanguageChoice;
};

export function I18nProvider({ children, language = "zh-CN" }: I18nProviderProps) {
  const value = useMemo(() => createI18n(language), [language]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  return useContext(I18nContext);
}
