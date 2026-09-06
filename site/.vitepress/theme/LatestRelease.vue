<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useData } from "vitepress";
import { PRODUCTS, latestUrl } from "../products";

/**
 * Latest release card for one product. Reads the public GitHub API in the
 * visitor's browser; the site never proxies or hosts the file, so what people
 * click is the same GitHub asset URL the app's own announcement points at.
 */
/**
 * `card` is the full download card (download page). `hero` is the landing
 * page's single primary button plus a text link for the bare exe.
 */
const props = withDefaults(defineProps<{ product?: string; variant?: "card" | "hero" }>(), {
  product: "maplelink",
  variant: "card",
});
const product = PRODUCTS[props.product];

interface Asset {
  name: string;
  browser_download_url: string;
  size: number;
}
interface Release {
  tag_name: string;
  html_url: string;
  body: string;
  published_at: string;
  assets: Asset[];
}

const { lang } = useData();
const release = ref<Release | null>(null);
const failed = ref(false);

const t = (zhTW: string, zhCN: string, en: string) =>
  lang.value === "zh-CN" ? zhCN : lang.value.startsWith("en") ? en : zhTW;

const find = (re: RegExp) => release.value?.assets.find((a) => re.test(a.name));
const setup = computed(() => find(product.setup));
const portable = computed(() => find(product.portable));
const sha256 = computed(() => release.value?.body.match(product.sha256)?.[1]);
const mb = (n: number) => `${(n / 1024 / 1024).toFixed(1)} MB`;
const version = computed(() => release.value?.tag_name.replace(/^v/, "").split(".").slice(0, 3).join("."));

onMounted(async () => {
  try {
    const res = await fetch(`https://api.github.com/repos/${product.repo}/releases/latest`);
    if (!res.ok) throw new Error(String(res.status));
    release.value = await res.json();
  } catch {
    failed.value = true;
  }
});
</script>

<template>
  <div v-if="variant === 'hero'" class="hero">
    <template v-if="release">
      <a class="hero__btn" :href="(setup ?? portable)?.browser_download_url ?? release.html_url">
        {{ t("下載", "下载", "Download") }} {{ product.name }} v{{ version }}
        <small v-if="setup ?? portable">{{ mb((setup ?? portable)!.size) }}</small>
      </a>
      <a v-if="setup && portable" class="hero__alt" :href="portable.browser_download_url">
        {{ t("免安裝版", "免安装版", "Portable") }} {{ portable.name }}
      </a>
    </template>
    <template v-else>
      <a class="hero__btn" :href="latestUrl(product)">
        {{ t("下載", "下载", "Download") }} {{ product.name }}
      </a>
    </template>
  </div>

  <div v-else class="release">
    <template v-if="release">
      <div class="release__head">
        <span class="release__name">{{ product.name }}</span>
        <span class="release__tag">v{{ version }}</span>
        <span class="release__date">{{ new Date(release.published_at).toLocaleDateString(lang) }}</span>
      </div>

      <div class="release__options">
        <div v-if="setup" class="release__option">
          <a class="release__btn" :href="setup.browser_download_url">
            <span>{{ t("下載", "下载", "Download") }}</span>
            <strong>{{ setup.name }}</strong>
            <small>{{ mb(setup.size) }}</small>
          </a>
          <p class="release__hint">
            {{
              t(
                "第一次使用選這個。解壓後得到一個資料夾，程式和說明都在其中。",
                "第一次使用选这个。解压后得到一个文件夹，程序和说明都在里面。",
                "Pick this if it is your first time. It unpacks a folder with the app and a readme.",
              )
            }}
          </p>
        </div>
        <div v-if="portable" class="release__option">
          <a class="release__btn release__btn--alt" :href="portable.browser_download_url">
            <span>{{ t("下載", "下载", "Download") }}</span>
            <strong>{{ portable.name }}</strong>
            <small>{{ mb(portable.size) }}</small>
          </a>
          <p class="release__hint">
            {{
              t(
                "只有程式本身。已在使用的人自動更新換的就是這個檔案。",
                "只有程序本身。已在使用的人自动更新换的就是这个文件。",
                "Just the program. This is the file auto-update replaces for existing users.",
              )
            }}
          </p>
        </div>
        <a v-if="!setup && !portable" class="release__btn" :href="release.html_url">
          <strong>{{ t("前往發佈頁", "前往发布页", "Open release page") }}</strong>
        </a>
      </div>

      <div v-if="sha256 && portable" class="release__sha">
        <span class="release__sha-label">SHA256 · {{ portable.name }}</span>
        <code>{{ sha256 }}</code>
      </div>
      <p class="release__foot">
        {{
          t(
            "按鈕直接連到 GitHub，本站不存放任何執行檔。",
            "按钮直接连到 GitHub，本站不存放任何可执行文件。",
            "Buttons link straight to GitHub. This site hosts no executables.",
          )
        }}
        <a :href="release.html_url">{{ t("發佈說明", "发布说明", "Release notes") }}</a>
      </p>
    </template>
    <template v-else-if="failed">
      <p class="release__hint">
        {{
          t(
            "暫時讀不到最新版本資訊，請直接到 GitHub 下載。",
            "暂时读不到最新版本信息，请直接到 GitHub 下载。",
            "Could not load the latest release. Download on GitHub instead.",
          )
        }}
      </p>
      <a class="release__btn" :href="latestUrl(product)">
        <strong>{{ t("前往 GitHub Releases", "前往 GitHub Releases", "Open GitHub Releases") }}</strong>
      </a>
    </template>
    <p v-else class="release__hint">
      {{ t("正在讀取最新版本…", "正在读取最新版本…", "Loading latest release…") }}
    </p>
  </div>
</template>

<style scoped>
.hero {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}
.hero__btn {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  padding: 14px 24px;
  border-radius: 12px;
  background: var(--vp-c-brand-1);
  color: #fff !important;
  font-size: 16px;
  font-weight: 600;
  text-decoration: none !important;
  transition: background 0.15s;
}
.hero__btn:hover {
  background: var(--vp-c-brand-2);
}
.hero__btn small {
  font-size: 13px;
  font-weight: 400;
  opacity: 0.8;
}
.hero__alt {
  font-size: 14px;
}
.release {
  border: 1px solid var(--vp-c-divider);
  border-radius: 14px;
  padding: 22px 24px;
  margin: 16px 0;
  background: var(--vp-c-bg-soft);
  text-align: left;
}
.release__head {
  display: flex;
  gap: 10px;
  align-items: baseline;
  margin-bottom: 18px;
}
.release__name {
  font-weight: 700;
  font-size: 1.05rem;
}
.release__tag {
  font-family: var(--vp-font-family-mono);
  font-size: 0.9rem;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
}
.release__date {
  color: var(--vp-c-text-3);
  font-size: 0.85rem;
  margin-left: auto;
}
.release__options {
  display: grid;
  gap: 18px;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
}
.release__option {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.release__btn {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
  padding: 12px 18px;
  border-radius: 10px;
  background: var(--vp-c-brand-1);
  color: #fff !important;
  text-decoration: none !important;
  line-height: 1.3;
  transition: background 0.15s;
}
.release__btn span {
  font-size: 0.75rem;
  opacity: 0.8;
  text-transform: uppercase;
  letter-spacing: 0.04em;
}
.release__btn strong {
  font-size: 1rem;
  word-break: break-all;
}
.release__btn small {
  font-size: 0.8rem;
  opacity: 0.85;
}
.release__btn:hover {
  background: var(--vp-c-brand-2);
}
.release__btn--alt {
  background: var(--vp-c-default-soft);
  color: var(--vp-c-text-1) !important;
  border: 1px solid var(--vp-c-divider);
}
.release__btn--alt:hover {
  background: var(--vp-c-default-3);
}
.release__hint {
  margin: 0;
  font-size: 0.85rem;
  line-height: 1.55;
  color: var(--vp-c-text-2);
}
.release__sha {
  margin: 20px 0 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 0.8rem;
}
.release__sha-label {
  color: var(--vp-c-text-3);
}
.release__sha code {
  word-break: break-all;
  font-size: 0.78rem;
}
.release__foot {
  margin: 14px 0 0;
  font-size: 0.8rem;
  color: var(--vp-c-text-3);
}
</style>
