export type ClipboardWriter = (text: string) => Promise<void>;

export const writeClipboardText: ClipboardWriter = async (text: string) => {
  const writeText = navigator.clipboard?.writeText;
  if (!writeText) {
    throw new Error("剪贴板不可用，请检查浏览器权限或 Tauri 剪贴板支持");
  }

  await writeText.call(navigator.clipboard, text);
};
