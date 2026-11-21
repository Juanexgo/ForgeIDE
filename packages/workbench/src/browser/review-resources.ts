import { injectable } from 'inversify';
import type { Resource, ResourceResolver } from '@theia/core/lib/common/resource.js';
import URI from '@theia/core/lib/common/uri.js';
/** Immutable snapshots; contents never appear in a URI or log. */
@injectable()
export class ReviewResources implements ResourceResolver {
 private readonly snapshots = new Map<string, string>();
 add(uri: URI, contents: string): void {if(new TextEncoder().encode(contents).length>2*1024*1024)throw new Error('Review snapshot exceeds 2 MiB');const key=uri.toString();this.snapshots.delete(key);this.snapshots.set(key,contents);while(this.snapshots.size>64)this.snapshots.delete(this.snapshots.keys().next().value!);}
 resolve(uri: URI): Resource {
  const contents = this.snapshots.get(uri.toString());
  if(uri.scheme !== 'forge-review' || contents === undefined) throw new Error('Not a Forge review snapshot');
  return {uri, readOnly:true, autosaveable:false, readContents:async()=>contents, dispose:()=>{}};
 }
}
