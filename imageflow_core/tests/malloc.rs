
// Test absurdly high malloc (address space exhaustion)
// Test malloc higher than swap, reduce until success
// Write zeroes to huge malloc, verify process doesn't crash (hey, overcommit isn't what you think)
// Free
// Malloc 1mb chunks until they fail. See if smaller mallocs succeed, and how many of them. Wait a bit, then try an extra 1mb chunck
// Have time limit
