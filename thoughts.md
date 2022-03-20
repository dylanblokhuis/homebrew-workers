## kv storage

```javascript
async function onRequest(event) {
  await KV.set("foo", "bar");

  const value = await KV.get("foo");

  event.respondWith(new Response(value);
}
```
