function required(env,names){const missing=names.filter(name=>typeof env[name]!=='string'||!env[name].trim());if(missing.length)throw Error('Missing signing configuration: '+missing.join(', '));}
function signedBuildConfig(base,platform,env=process.env){
 const result={...base,forceCodeSigning:true,directories:{...base.directories,output:'../../dist-signed'}};
 if(platform==='darwin'){
  required(env,['CSC_LINK','CSC_KEY_PASSWORD','MAC_SIGNING_IDENTITY','APPLE_ID','APPLE_APP_SPECIFIC_PASSWORD','APPLE_TEAM_ID']);
  if(!env.MAC_SIGNING_IDENTITY.startsWith('Developer ID Application: '))throw Error('Distribution requires Developer ID Application; Apple Development and ad-hoc identities are not accepted');
  return {...result,mac:{...base.mac,identity:env.MAC_SIGNING_IDENTITY,hardenedRuntime:true,notarize:true,forceCodeSigning:true,target:['dmg','zip']},dmg:{...base.dmg,sign:true}};
 }
 if(platform==='win32'){
  required(env,['WIN_CSC_LINK','WIN_CSC_KEY_PASSWORD','WIN_PUBLISHER_NAME']);
  return {...result,win:{...base.win,forceCodeSigning:true,signAndEditExecutable:true,verifyUpdateCodeSignature:true,signtoolOptions:{...base.win?.signtoolOptions,publisherName:env.WIN_PUBLISHER_NAME,signingHashAlgorithms:['sha256']},target:['nsis']}};
 }
 throw Error('Signed distribution supports macOS and Windows runners only');
}
module.exports={signedBuildConfig};
