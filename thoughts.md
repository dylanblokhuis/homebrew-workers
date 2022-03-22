## kv storage

use stores table and store the kv values here with the namespace

```javascript
async function onRequest(event) {
  await KV.NAMESPACE.set("foo", "bar");

  const value = await KV.NAMESPACE.get("foo");

  event.respondWith(new Response(value);
}
```

## roadmap?

1. implement admin route to create users
2. implement script uploading
3. implement user route to create namespaces
4. implement kv ops for workers
