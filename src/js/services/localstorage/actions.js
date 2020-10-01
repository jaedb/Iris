export function restoreFromColdStore(item) {
  return {
    type: 'RESTORE_FROM_COLD_STORE',
    item,
  };
}
