# Cert de testsigning para firmar netr28ux_patched.sys (lab-only)
$ErrorActionPreference = 'Stop'
$pw = ConvertTo-SecureString -String 'uifipill-lab' -Force -AsPlainText
# 1) crear cert self-signed con private key en CurrentUser\My
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject 'CN=UIFIPILL Lab Test' `
    -KeyUsage DigitalSignature, CertSign -KeyAlgorithm RSA -KeyLength 2048 `
    -NotAfter (Get-Date).AddYears(5) -CertStoreLocation 'Cert:\CurrentUser\My'
Write-Host "CERT_THUMBPRINT: $($cert.Thumbprint)"
# 2) exportar .cer (publico) e importarlo en TrustedPublisher + Root de la maquina local
$cer = "$PWD\labtest.cer"
Export-Certificate -Cert $cert -FilePath $cer | Out-Null
Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\LocalMachine\TrustedPublisher' | Out-Null
Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\LocalMachine\Root' | Out-Null
# 3) exportar .pfx para signtool
Export-PfxCertificate -Cert $cert -FilePath "$PWD\labtest.pfx" -Password $pw | Out-Null
Write-Host 'OK cert creado y confiado (TrustedPublisher + Root LocalMachine)'
