import * as React from 'react';import type {ForgeService} from '../common/forge-protocol.js';
export function LanguagePanel({api}:{api:ForgeService}):React.ReactElement {
 const [profiles,setProfiles]=React.useState<Awaited<ReturnType<ForgeService['languageTools']>>>([]),[error,setError]=React.useState('');
 async function refresh():Promise<void>{try{setProfiles(await api.languageTools());setError('');}catch(e){setError(String(e));}}
 React.useEffect(()=>{void refresh();},[]);
 return <article className="forge-card"><h3>Herramientas de lenguaje</h3><p>Detecta ejecutables sin lanzarlos ni instalar toolchains automáticamente. Que un archivo exista no confirma su versión, su configuración o que el servidor de lenguaje funcione. Instala y configura las extensiones desde Open VSX.</p><button onClick={()=>void refresh()}>Revisar herramientas</button>{error&&<p role="alert">{error}</p>}{profiles.map(p=><div className="forge-card" key={p.language}><strong>{p.language}</strong><p className="forge-muted">Extensión: {p.extension}</p>{p.tools.map(t=><p className="forge-path" key={t.name}>{t.name}: {t.path||'No encontrado en las rutas conocidas'}</p>)}</div>)}</article>;
}
