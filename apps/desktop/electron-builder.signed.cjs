const {signedBuildConfig}=require('../../scripts/signed-build-config.cjs');
module.exports=signedBuildConfig(require('./package.json').build,process.platform);
