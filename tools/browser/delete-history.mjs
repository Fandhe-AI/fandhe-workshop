// 保存済みの履歴の削除の試験が共有する補助関数（TASK-341。SAVE-32・SHELL-38）。
//
// 役割: 保存先（localStorage・IndexedDB）の観察と書き込み、試験の口（`deleteHistory` など）の送信、削除の結果の待ちを一か所に置く。
// `.test.mjs` は import すると試験が登録されるため、共有する部分をここへ切り出した。
// 呼び出し元: delete-history.test.mjs（中核の試験）、delete-history-scenes.test.mjs（「やめる」と通信 0 件の試験）。
// 返すのは保存先の写しで、鍵・値を含む。呼び出し側は Node 側の出力・失敗の文に鍵・値を出さない（有無・本数・比較の結果だけを出す）。

const T = 30000;

export const localEntries = (page) => page.evaluate(() => {
  const out = {};
  for (let i = 0; i < localStorage.length; i += 1) {
    const k = localStorage.key(i);
    out[k] = localStorage.getItem(k);
  }
  return out;
});

export const press = (page, name) => page.evaluate((n) => window.__fandheTest[n](), name);
export const waitDeleteResult = (page) => page.waitForFunction(
  () => JSON.parse(window.__fandheTest.stateJson()).engine.lastDelete !== null,
  undefined,
  { timeout: T },
);

export const idbDump = (page) => page.evaluate(() => new Promise((resolve, reject) => {
  const req = indexedDB.open('fandhe-workshop', 1);
  req.onerror = () => reject(new Error('idb open failed'));
  req.onupgradeneeded = () => { req.result.createObjectStore('history'); };
  req.onsuccess = () => {
    const db = req.result;
    const tx = db.transaction('history', 'readonly');
    const store = tx.objectStore('history');
    const keys = store.getAllKeys();
    const vals = store.getAll();
    tx.oncomplete = () => {
      const out = {};
      keys.result.forEach((k, i) => { out[k] = vals.result[i]; });
      db.close();
      resolve(out);
    };
    tx.onerror = () => reject(new Error('idb read failed'));
  };
}));

export const idbPut = (page, entries) => page.evaluate((es) => new Promise((resolve, reject) => {
  const req = indexedDB.open('fandhe-workshop', 1);
  req.onerror = () => reject(new Error('idb open failed'));
  req.onupgradeneeded = () => { req.result.createObjectStore('history'); };
  req.onsuccess = () => {
    const db = req.result;
    const tx = db.transaction('history', 'readwrite');
    const store = tx.objectStore('history');
    for (const [k, v] of Object.entries(es)) store.put(v, k);
    tx.oncomplete = () => { db.close(); resolve(true); };
    tx.onerror = () => reject(new Error('idb write failed'));
  };
}), entries);

export const sameScopeKey = (key, mutate) => {
  const parts = key.split('/');
  mutate(parts);
  return parts.join('/');
};
