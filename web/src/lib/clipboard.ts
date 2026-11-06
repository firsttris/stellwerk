/** Whether the text reached the clipboard; `navigator.clipboard` needs a secure origin. */
export async function copyToClipboard(text: string): Promise<boolean> {
  if (!navigator.clipboard) return false;
  return navigator.clipboard.writeText(text).then(
    () => true,
    () => false,
  );
}
