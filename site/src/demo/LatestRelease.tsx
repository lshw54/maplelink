import React from "react";
import clsx from "clsx";
import { useLocale, useT } from "./i18n";
import { PRODUCTS, latestUrl } from "./products";
import { useLatestRelease, useLatestVersion } from "./release";
import styles from "./LatestRelease.module.css";

/**
 * Latest release card for one product. Reads the public GitHub API in the
 * visitor's browser; the site never proxies or hosts the file, so what people
 * click is the same GitHub asset URL the app's own announcement points at.
 *
 * `card` is the full download card (download page). `hero` is the landing
 * page's single primary button plus a text link for the bare exe.
 */
export default function LatestRelease({
  product: productId = "maplelink",
  variant = "card",
}: {
  product?: string;
  variant?: "card" | "hero";
}) {
  const product = PRODUCTS[productId];
  const lang = useLocale();
  const t = useT();
  const { release, failed } = useLatestRelease(productId);
  const version = useLatestVersion(productId);

  const find = (re: RegExp) => release?.assets.find((a) => re.test(a.name));
  const setup = find(product.setup);
  const portable = find(product.portable);
  const sha256 = release?.body.match(product.sha256)?.[1];
  const mb = (n: number) => `${(n / 1024 / 1024).toFixed(1)} MB`;

  if (variant === "hero") {
    const primary = setup ?? portable;
    return (
      <div className={styles.hero}>
        {release ? (
          <>
            <a className={styles.hero__btn} href={primary?.browser_download_url ?? release.html_url}>
              {t("下載", "下载", "Download")} {product.name} v{version}
              {primary && <small>{mb(primary.size)}</small>}
            </a>
            {setup && portable && (
              <a className={styles.hero__alt} href={portable.browser_download_url}>
                {t("免安裝版", "免安装版", "Portable")} {portable.name}
              </a>
            )}
          </>
        ) : (
          <a className={styles.hero__btn} href={latestUrl(product)}>
            {t("下載", "下载", "Download")} {product.name}
          </a>
        )}
      </div>
    );
  }

  return (
    <div className={clsx("release", styles.release)}>
      {release ? (
        <>
          <div className={styles.release__head}>
            <span className={styles.release__name}>{product.name}</span>
            <span className={styles.release__tag}>v{version}</span>
            <span className={styles.release__date}>{new Date(release.published_at).toLocaleDateString(lang)}</span>
          </div>

          <div className={styles.release__options}>
            {setup && (
              <div className={styles.release__option}>
                <a className={styles.release__btn} href={setup.browser_download_url}>
                  <span>{t("下載", "下载", "Download")}</span>
                  <strong>{setup.name}</strong>
                  <small>{mb(setup.size)}</small>
                </a>
                <p className={styles.release__hint}>
                  {t(
                    "第一次使用選這個。解壓後得到一個資料夾，程式和說明都在其中。",
                    "第一次使用选这个。解压后得到一个文件夹，程序和说明都在里面。",
                    "Pick this if it is your first time. It unpacks a folder with the app and a readme.",
                  )}
                </p>
              </div>
            )}
            {portable && (
              <div className={styles.release__option}>
                <a className={clsx(styles.release__btn, styles["release__btn--alt"])} href={portable.browser_download_url}>
                  <span>{t("下載", "下载", "Download")}</span>
                  <strong>{portable.name}</strong>
                  <small>{mb(portable.size)}</small>
                </a>
                <p className={styles.release__hint}>
                  {t(
                    "只有程式本身。已在使用的人自動更新換的就是這個檔案。",
                    "只有程序本身。已在使用的人自动更新换的就是这个文件。",
                    "Just the program. This is the file auto-update replaces for existing users.",
                  )}
                </p>
              </div>
            )}
            {!setup && !portable && (
              <a className={styles.release__btn} href={release.html_url}>
                <strong>{t("前往發佈頁", "前往发布页", "Open release page")}</strong>
              </a>
            )}
          </div>

          {sha256 && portable && (
            <div className={styles.release__sha}>
              <span className={styles["release__sha-label"]}>SHA256 · {portable.name}</span>
              <code>{sha256}</code>
            </div>
          )}
          <p className={styles.release__foot}>
            {t(
              "按鈕直接連到 GitHub，本站不存放任何執行檔。",
              "按钮直接连到 GitHub，本站不存放任何可执行文件。",
              "Buttons link straight to GitHub. This site hosts no executables.",
            )}{" "}
            <a href={release.html_url}>{t("發佈說明", "发布说明", "Release notes")}</a>
          </p>
        </>
      ) : failed ? (
        <>
          <p className={styles.release__hint}>
            {t(
              "暫時讀不到最新版本資訊，請直接到 GitHub 下載。",
              "暂时读不到最新版本信息，请直接到 GitHub 下载。",
              "Could not load the latest release. Download on GitHub instead.",
            )}
          </p>
          <a className={styles.release__btn} href={latestUrl(product)}>
            <strong>{t("前往 GitHub Releases", "前往 GitHub Releases", "Open GitHub Releases")}</strong>
          </a>
        </>
      ) : (
        <p className={styles.release__hint}>{t("正在讀取最新版本…", "正在读取最新版本…", "Loading latest release…")}</p>
      )}
    </div>
  );
}
