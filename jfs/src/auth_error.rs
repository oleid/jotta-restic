/*
<error>
  <code>401</code>
  <message>org.springframework.security.authentication.BadCredentialsException: Bad credentials</message>
  <reason>Unauthorized</reason>
  <cause></cause>
  <hostname>dn-125</hostname>
  <x-id>096492164813</x-id>
</error>

*/

struct AuthError {
    message: String,
    code: usize,
}
