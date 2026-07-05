export type ContentLanguage = "Chinese" | "Other";

const chinesePattern = /[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff]/;

export function detectContentLanguage(content: string): ContentLanguage {
  return isChineseContent(content) ? "Chinese" : "Other";
}

export function isChineseContent(content: string): boolean {
  return chinesePattern.test(content);
}
