## kv storage

use stores table and store the kv values here with the namespace

```javascript
async function onRequest(event) {
  await KV.NAMESPACE.set("foo", "bar");

  const value = await KV.NAMESPACE..get("foo");

  event.respondWith(new Response(value);
}
```
