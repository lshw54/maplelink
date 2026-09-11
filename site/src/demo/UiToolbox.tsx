import React, { type ReactNode } from "react";
import clsx from "clsx";
import { useT } from "./i18n";
import UiTitlebar from "./UiTitlebar";
import styles from "./UiToolbox.module.css";

/**
 * The toolbox window shell (750×490): title bar, the 150px tab rail on the
 * left, a back button at its foot, and a scrolling content area. Tabs are
 * clickable; the parent decides which one is shown.
 */
export default function UiToolbox({
  tab,
  onTabChange,
  children,
  titlebarHint = null,
  onTitlebarClient,
}: {
  tab: string;
  onTabChange: (key: string) => void;
  children?: ReactNode;
  /** Passed through to the title bar, for demos about a shortcut up there. */
  titlebarHint?: "client" | null;
  onTitlebarClient?: () => void;
}) {
  const t = useT();

  const TABS = [
    { key: "tools", icon: "🛠", label: () => t("工具", "工具", "Tools") },
    { key: "announcements", icon: "📢", label: () => t("公告", "公告", "Announcements") },
    {
      key: "account_manager",
      icon: "👤",
      label: () => t("帳號管理", "账号管理", "Account Manager"),
    },
    { key: "settings", icon: "⚙", label: () => t("設定", "设置", "Settings") },
    { key: "advanced", icon: "🔧", label: () => t("進階", "高级", "Advanced") },
    { key: "about", icon: "ℹ", label: () => t("關於", "关于", "About") },
  ];

  return (
    <div className={clsx("ml", styles.tbx)}>
      <div className="ml-glow"></div>
      <UiTitlebar page="main" region="HK" hint={titlebarHint} onClient={onTitlebarClient} />
      <div className={styles.tbx__body}>
        <nav className={styles.tbx__nav}>
          {TABS.map((x) => (
            <button
              key={x.key}
              className={clsx(styles.tbx__tab, x.key === tab && styles["tbx__tab--on"])}
              onClick={() => onTabChange(x.key)}
            >
              <span className={styles.tbx__icon}>{x.icon}</span>
              {x.label()}
            </button>
          ))}
          <div className={styles.tbx__spacer}></div>
          <span className={styles.tbx__back}>{t("返回", "返回", "Back")}</span>
        </nav>
        <div className={styles.tbx__content}>{children}</div>
      </div>
    </div>
  );
}
