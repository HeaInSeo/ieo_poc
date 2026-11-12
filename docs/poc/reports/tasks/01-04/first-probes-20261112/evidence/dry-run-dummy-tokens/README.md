# 하네스 사전 점검(가짜 토큰, 모델 요청 0회)

실제 probe 전에 하네스를 점검했다. 가짜 토큰(`sk-ant-DUMMY…`)과 임시 경로를 쓰고 `setup`, `p2-files`만 실행했다. **이 결과는 P2 판정 증거가 아니다.** P2는 실제 자격으로 다시 실행해 판정한다.

- 첫 실행에서 하네스 결함이 드러났다. 볼륨을 SELinux 라벨 없이 마운트하자 역할 컨테이너가 **자기** 볼륨에도 접근하지 못했다(SELinux enforcing). 이 상태라면 2b의 “거부”가 UID 경계가 아니라 SELinux에서 나왔을 수 있다.
- 교정 1: 모든 역할 볼륨을 공유 라벨 `:z`로 마운트해 SELinux가 역할 사이 접근을 결정하지 않게 했다.
- 교정 2: 양성 대조를 넣었다. 2p는 소유 UID가 자기 볼륨을 읽고 쓰는지 본다. 2d-control은 같은 검사로 자기 토큰을 찾는지 본다.
- 교정 후 결과([P2-files.json](P2-files.json)):
  - 2p 양쪽은 LIST/READ/WRITE_OK이고, 2b 양방향은 모두 DENIED다.
  - 2d-control은 TOKEN_PRESENT, 2d는 TOKEN_ABSENT다.
  - 2a 쓰기는 호스트 Guardrail 볼륨에 나타나지 않았다(INTRUSION_ABSENT).
  - 2e에서 호스트 heain은 READ_DENIED다.
- 이 결과는 수단 가용성과 하네스의 동작을 보여 줄 뿐이다. 실제 역할 간 차단 성공(P2 판정)은 실제 자격으로 실행한 결과로만 기록한다.
